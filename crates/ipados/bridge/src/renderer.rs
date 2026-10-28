//! UIKit surface integration. CAD scene creation, camera math and drawing pipelines
//! stay in the shared Rust crates; this module owns only the surface and its lifetime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use solidcraft_gpu::view::{BlockBytes, BlockTris, BodySpan, DrawStats, Frame, GpuScene, GpuTarget, Renderer, SceneBlock, SceneSlot, TRI_SIZE};
use solidcraft_ipados_session::RenderSnapshot;

/// Surface calls are serialized on the host's display worker/render lock. UIKit creates
/// and retains the CAMetalLayer on the main thread; this boundary uses the layer
/// directly, never a UIView. The host retains it until the surface is destroyed.
pub struct NativeRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    depth: Option<wgpu::Texture>,
    size: [u32; 2],
    active: bool,
    lost: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
    uploaded: Option<(String, u64)>,
    scene_key: u64,
    scene: SceneSlot,
    guides: SceneSlot,
    highlight: SceneSlot,
    empty: SceneSlot,
    overlay_revision: u64,
    highlight_revision: u64,
    bodies: Vec<(String, BlockTris)>,
}

/// A skipped frame (inactive, occluded, zero size or transient drawable failure)
/// is a successful lifecycle operation, but is never evidence of presentation.
#[derive(Default)]
pub struct DrawOutcome {
    pub presented: bool,
    pub stats: DrawStats,
}

impl NativeRenderer {
    pub fn new(instance: &wgpu::Instance, surface: wgpu::Surface<'static>, width: u32, height: u32) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            ..Default::default()
        }))
        .map_err(|e| format!("Metal adapter: {e}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("SolidCraft iPadOS"),
            required_features: solidcraft_gpu::depth_features(&adapter),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| format!("Metal device: {e}"))?;
        let config = surface.get_default_config(&adapter, width.max(1), height.max(1)).ok_or("Metal surface has no compatible configuration")?;
        let lost = Arc::new(AtomicBool::new(false));
        let lost_callback = lost.clone();
        device.set_device_lost_callback(move |_, _| lost_callback.store(true, Ordering::Release));
        let error: Arc<Mutex<Option<String>>> = Arc::default();
        let error_callback = error.clone();
        device.on_uncaptured_error(Arc::new(move |e| {
            if let Ok(mut message) = error_callback.lock() {
                *message = Some(e.to_string());
            }
        }));
        let target =
            GpuTarget { format: config.format, depth: Some(solidcraft_gpu::depth_format(&device, true)), samples: 1, tess: false, oit: false };
        let scopes = scopes(&device);
        let renderer = Renderer::new(&device, target, None);
        finish_scopes(scopes)?;
        let mut state = Self {
            surface,
            device,
            queue,
            config,
            renderer,
            depth: None,
            size: [0, 0],
            active: true,
            lost,
            error,
            uploaded: None,
            scene_key: 0,
            scene: Arc::new(Mutex::new(None)),
            guides: Arc::new(Mutex::new(None)),
            highlight: Arc::new(Mutex::new(None)),
            empty: Arc::new(Mutex::new(None)),
            overlay_revision: 0,
            highlight_revision: 0,
            bodies: Vec::new(),
        };
        state.resize(width, height)?;
        Ok(state)
    }

    /// Zero sized views retain their session but do not configure or acquire a drawable.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        self.check_device()?;
        if width > self.device.limits().max_texture_dimension_2d || height > self.device.limits().max_texture_dimension_2d {
            return Err("viewport exceeds the Metal device's texture size limit".into());
        }
        self.size = [width, height];
        self.depth = None;
        if width == 0 || height == 0 || !self.active {
            return Ok(());
        }
        self.config.width = width;
        self.config.height = height;
        let scopes = scopes(&self.device);
        self.surface.configure(&self.device, &self.config);
        let format = self.renderer.target().depth.ok_or("viewport depth format is unavailable")?;
        self.depth = Some(self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SolidCraft iPadOS depth"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        }));
        finish_scopes(scopes)
    }

    /// UIKit resign-active arrives before backgrounding. No further GPU commands
    /// are submitted until become-active, when the drawable is configured again.
    pub fn set_active(&mut self, active: bool) -> Result<(), String> {
        if self.active == active {
            return self.check_device();
        }
        self.active = active;
        if !active {
            self.device
                .poll(wgpu::PollType::Wait { submission_index: None, timeout: Some(std::time::Duration::from_secs(1)) })
                .map_err(|e| format!("Metal suspension could not drain queued work: {e}"))?;
        }
        self.resize(self.size[0], self.size[1])
    }

    pub fn draw(&mut self, snapshot: &RenderSnapshot) -> Result<DrawOutcome, String> {
        if !self.active || self.size.contains(&0) {
            return Ok(DrawOutcome::default());
        }
        self.check_device()?;
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                // Drop the acquired texture before reconfiguration.
                drop(texture);
                self.resize(self.size[0], self.size[1])?;
                return Ok(DrawOutcome::default());
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(DrawOutcome::default()),
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.size[0], self.size[1])?;
                return Ok(DrawOutcome::default());
            }
            wgpu::CurrentSurfaceTexture::Lost => return Err("Metal surface lost; recreate the renderer while retaining the CAD session".into()),
            wgpu::CurrentSurfaceTexture::Validation => return Err("Metal drawable acquisition failed validation".into()),
        };
        if self.uploaded.as_ref().is_none_or(|(id, revision)| id != &snapshot.model.document_id || *revision != snapshot.geometry_revision) {
            self.scene_key = self.scene_key.checked_add(1).ok_or("render scene revision exhausted")?;
            let (model, bodies) = model_geometry(snapshot, self.scene_key)?;
            *self.scene.lock().map_err(|_| "render scene lock is poisoned")? = Some(model);
            self.bodies = bodies;
            self.uploaded = Some((snapshot.model.document_id.clone(), snapshot.geometry_revision));
        }
        if self.overlay_revision != snapshot.guide_revision {
            *self.guides.lock().map_err(|_| "guide scene lock poisoned")? = Some(solidcraft_gpu::capture::scene_geometry(&snapshot.guides));
            self.overlay_revision = snapshot.guide_revision;
        }
        if self.highlight_revision != snapshot.highlight_revision {
            *self.highlight.lock().map_err(|_| "highlight scene lock poisoned")? = Some(highlight_geometry(snapshot, &self.bodies));
            self.highlight_revision = snapshot.highlight_revision;
        }
        let frame = self.frame(snapshot);
        let view = output.texture.create_view(&Default::default());
        let depth = self.depth.as_ref().ok_or("viewport depth texture is unavailable")?.create_view(&Default::default());
        let scopes = scopes(&self.device);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("SolidCraft iPadOS frame") });
        self.renderer.prepare(&self.device, &self.queue, &mut encoder, &frame);
        let background = snapshot.model.scene.background.map_or(wgpu::Color::TRANSPARENT, |(top, _)| {
            let [r, g, b] =
                if self.config.format.is_srgb() { [top.0, top.1, top.2].map(solidcraft_render::surface::srgb8_to_linear) } else { top.f() };
            wgpu::Color { r, g, b, a: 1.0 }
        });
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("SolidCraft iPadOS viewport"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(background), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(solidcraft_gpu::view::far_depth(self.renderer.target().reversed_depth())),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }),
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                })
                .forget_lifetime();
            self.renderer.paint(&frame, &mut pass, [0.0, 0.0, self.size[0] as f32, self.size[1] as f32]);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(output);
        finish_scopes(scopes)?;
        self.check_device()?;
        let stats = *self.renderer.stats().lock().map_err(|_| "render statistics lock is poisoned")?;
        Ok(DrawOutcome { presented: true, stats })
    }

    fn frame(&self, snapshot: &RenderSnapshot) -> Frame {
        let camera = &snapshot.camera;
        let eye = camera.eye().to_f32();
        Frame {
            product: false,
            key: self.scene_key,
            slot: self.scene.clone(),
            sketch_key: snapshot.guide_revision,
            sketch_slot: self.guides.clone(),
            hl_key: snapshot.highlight_revision,
            hl_slot: self.highlight.clone(),
            pv_key: 0,
            pv_slot: self.empty.clone(),
            pv_hide: false,
            view_proj: snapshot.view_proj.to_f32(),
            back: camera.back().to_f32(),
            size_px: [self.size[0] as f32, self.size[1] as f32],
            clip: [0.0; 4],
            cap: [0.0; 4],
            images: Vec::new(),
            analysis: [0.0; 8],
            analysis_colors: [0.0; 36],
            access: None,
            outline: true,
            eye: [eye[0], eye[1], eye[2], if camera.fov > 0.0 { 1.0 } else { 0.0 }],
            ao_radius: 0.0,
            ssaa: 1.0,
            hover_tint: snapshot.hover_tint,
            selection_tint: snapshot.selection_tint,
        }
    }

    fn check_device(&self) -> Result<(), String> {
        if self.lost.load(Ordering::Acquire) {
            return Err("Metal device lost; recreate the renderer while retaining the CAD session".into());
        }
        if let Some(error) = self.error.lock().map_err(|_| "GPU error lock is poisoned")?.take() {
            return Err(format!("Metal: {error}"));
        }
        Ok(())
    }
}

fn scopes(device: &wgpu::Device) -> [wgpu::ErrorScopeGuard; 3] {
    [wgpu::ErrorFilter::OutOfMemory, wgpu::ErrorFilter::Internal, wgpu::ErrorFilter::Validation].map(|kind| device.push_error_scope(kind))
}

fn finish_scopes(scopes: [wgpu::ErrorScopeGuard; 3]) -> Result<(), String> {
    let mut message = None;
    for scope in scopes.into_iter().rev() {
        if let Some(error) = pollster::block_on(scope.pop()) {
            message = Some(error.to_string());
        }
    }
    message.map_or(Ok(()), Err)
}

/// Retain the shared converter's vertices in a shared-renderer block. Whole-body
/// feedback can then reference the model's GPU buffer, including an edgeless sphere.
/// No triangle is regenerated or uploaded just to change body hover/selection.
pub(crate) fn model_geometry(snapshot: &RenderSnapshot, key: u64) -> Result<(GpuScene, Vec<(String, BlockTris)>), String> {
    let mut scene = solidcraft_gpu::capture::scene_geometry(&snapshot.model.scene);
    let opaque = u32::try_from(scene.tris.len() / TRI_SIZE).map_err(|_| "model exceeds GPU vertex range")?;
    let glass = u32::try_from(scene.glass.len() / TRI_SIZE).map_err(|_| "model exceeds GPU vertex range")?;
    let total = opaque.checked_add(glass).ok_or("model exceeds GPU vertex range")?;
    let mut bytes = std::mem::take(&mut scene.tris);
    bytes.append(&mut scene.glass);
    let (mut solid_at, mut glass_at) = (0u32, opaque);
    let mut spans = Vec::new();
    let mut bodies = Vec::new();
    for (mesh, body) in snapshot.model.scene.meshes.iter().zip(&snapshot.model.picks.body_names()) {
        let triangles = mesh.mesh.triangles.iter().filter(|t| mesh.mesh.tri(t).is_some()).count();
        let vertices = u32::try_from(triangles).ok().and_then(|n| n.checked_mul(3)).ok_or("body exceeds GPU vertex range")?;
        let at = if mesh.glass() { &mut glass_at } else { &mut solid_at };
        let end = at.checked_add(vertices).ok_or("body exceeds GPU vertex range")?;
        let range = *at..end;
        let bounds = mesh.mesh.bounds();
        if !range.is_empty() {
            spans.push(BodySpan { vertices: range.clone(), bounds: [bounds.min.to_f32(), bounds.max.to_f32()] });
        }
        bodies.push((body.clone(), BlockTris { block: key, tris: vec![range.clone()], vertices: range }));
        *at = end;
    }
    if solid_at != opaque || glass_at != total {
        return Err("body ranges do not match the shared renderer's vertices".into());
    }
    spans.sort_by_key(|span| span.vertices.start);
    scene.blocks.push(SceneBlock {
        key,
        bytes: Arc::new(BlockBytes {
            vertices: Arc::new(bytes),
            bodies: Arc::new(spans),
            opaque: Arc::new((opaque > 0).then_some(0..opaque).into_iter().collect()),
            glass: Arc::new((glass > 0).then_some(opaque..total).into_iter().collect()),
            ..Default::default()
        }),
    });
    Ok((scene, bodies))
}

pub(crate) fn highlight_geometry(snapshot: &RenderSnapshot, bodies: &[(String, BlockTris)]) -> GpuScene {
    let mut highlight = solidcraft_gpu::capture::scene_geometry(&snapshot.highlight);
    highlight.selected_blocks = bodies.iter().filter(|(name, _)| snapshot.selected_bodies.contains(name)).map(|(_, tris)| tris.clone()).collect();
    highlight.hover_blocks = bodies.iter().filter(|(name, _)| snapshot.hovered_bodies.contains(name)).map(|(_, tris)| tris.clone()).collect();
    highlight
}

/// Offscreen proof of this adapter's exact model-block and highlight references,
/// using the shared viewport/capture pipelines; no native surface or fallback mesh.
pub fn capture_feedback(snapshot: &RenderSnapshot, capturer: &solidcraft_gpu::capture::Capturer, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (model, bodies) = model_geometry(snapshot, 1)?;
    let highlight = highlight_geometry(snapshot, &bodies);
    capturer
        .rgba_viewport(&snapshot.model.scene, &snapshot.camera, [width, height], model, highlight, (snapshot.hover_tint, snapshot.selection_tint))
        .map_err(|e| e.to_string())
}
