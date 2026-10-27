//! UIKit surface integration. CAD scene creation, camera math and drawing pipelines
//! stay in the shared Rust crates; this module owns only the surface and its lifetime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use solidcraft_gpu::view::{DrawStats, Frame, GpuTarget, Renderer, SceneSlot};
use solidcraft_ipados_session::RenderSnapshot;

/// All calls are serialized on the host's render/evaluation worker. UIKit creates
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
        let key = (snapshot.document_id.clone(), snapshot.scene_revision);
        if self.uploaded.as_ref() != Some(&key) {
            self.scene_key = self.scene_key.checked_add(1).ok_or("render scene revision exhausted")?;
            *self.scene.lock().map_err(|_| "render scene lock is poisoned")? = Some(solidcraft_gpu::capture::scene_geometry(&snapshot.scene));
            self.uploaded = Some(key);
        }
        let frame = frame(snapshot, self.size, self.scene_key, self.scene.clone());
        let view = output.texture.create_view(&Default::default());
        let depth = self.depth.as_ref().ok_or("viewport depth texture is unavailable")?.create_view(&Default::default());
        let scopes = scopes(&self.device);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("SolidCraft iPadOS frame") });
        self.renderer.prepare(&self.device, &self.queue, &mut encoder, &frame);
        let background = snapshot.scene.background.map_or(wgpu::Color::TRANSPARENT, |(top, _)| {
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

/// The small native foundation enables only the shared model scene and camera.
/// Sketch, hover, preview, analysis and gesture behavior are subsequent work.
fn frame(snapshot: &RenderSnapshot, [width, height]: [u32; 2], key: u64, scene: SceneSlot) -> Frame {
    let camera = &snapshot.camera;
    let empty = || Arc::new(Mutex::new(None));
    let eye = camera.eye().to_f32();
    Frame {
        product: false,
        key,
        slot: scene,
        sketch_key: 0,
        sketch_slot: empty(),
        hl_key: 0,
        hl_slot: empty(),
        pv_key: 0,
        pv_slot: empty(),
        pv_hide: false,
        view_proj: camera.view_proj(f64::from(width) / f64::from(height.max(1)), snapshot.scene.radius.max(1.0)).to_f32(),
        back: camera.back().to_f32(),
        size_px: [width as f32, height as f32],
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
        hover_tint: [0.0; 3],
        selection_tint: [0.0; 3],
    }
}
