//! UIKit surface integration. CAD scene creation, camera math and drawing pipelines
//! stay in the shared Rust crates; this module owns only the surface and its lifetime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use solidcraft_gpu::view::{
    BlockBytes, BlockTris, BodySpan, DrawStats, Frame, GpuScene, GpuTarget, InstanceLook, InstancedMesh, Renderer, SceneBlock, SceneSlot, SharedTris,
    TRI_SIZE,
};
use solidcraft_ipados_session::{NativeMeshInstance, RenderSnapshot};

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
    uploaded: Option<Arc<solidcraft_ipados_session::SceneSnapshot>>,
    scene_key: u64,
    scene: SceneSlot,
    guides: SceneSlot,
    highlight: SceneSlot,
    empty: SceneSlot,
    overlay_revision: u64,
    highlight_revision: u64,
    bodies: ModelBodies,
    meshes: NativeMeshCache,
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
            meshes: NativeMeshCache::default(),
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
        if self.uploaded.as_ref().is_none_or(|model| !Arc::ptr_eq(model, &snapshot.model)) {
            self.scene_key = self.scene_key.checked_add(1).ok_or("render scene revision exhausted")?;
            let (model, bodies) = model_geometry_cached(snapshot, self.scene_key, &mut self.meshes)?;
            *self.scene.lock().map_err(|_| "render scene lock is poisoned")? = Some(model);
            self.bodies = bodies;
            self.uploaded = Some(snapshot.model.clone());
            self.highlight_revision = 0;
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

/// Whole-body feedback keeps either the original fallback block or the exact
/// instance placement. The public GPU renderer owns both draw paths.
pub(crate) enum BodyGeometry {
    Block(BlockTris),
    Instance { key: u64, tris: Arc<SharedTris>, rows: [[f32; 4]; 3] },
}
pub(crate) type ModelBodies = Vec<(String, BodyGeometry)>;

struct CachedMesh {
    source: NativeMeshInstance,
    key: u64,
    used: u64,
    tris: Option<Arc<SharedTris>>,
}

/// Retained base Arcs prevent address-reuse aliases. Keys never rewind, including
/// after eviction; only immutable native meshes and their public GPU conversion
/// live here. The shared GPU renderer retains uploads while SharedTris returns.
#[derive(Default)]
pub(crate) struct NativeMeshCache {
    entries: Vec<CachedMesh>,
    next_key: u64,
    access: u64,
}
impl NativeMeshCache {
    pub(crate) fn shared(&mut self, source: &NativeMeshInstance) -> Result<Option<(u64, Arc<SharedTris>)>, String> {
        self.access = self.access.checked_add(1).ok_or("native mesh cache revision exhausted")?;
        if let Some(entry) = self.entries.iter_mut().find(|entry| Arc::ptr_eq(&entry.source.mesh, &source.mesh)) {
            entry.used = self.access;
            return Ok(entry.tris.as_ref().map(|tris| (entry.key, tris.clone())));
        }
        self.next_key = self.next_key.checked_add(1).ok_or("native mesh cache key exhausted")?;
        // The shared fallback converter repairs missing/invalid normals. Preserve
        // that path rather than changing the appearance of malformed mesh inputs.
        let finite = |v: &solidcraft_ipados_session::WorldPoint| v.is_finite() && [v.x, v.y, v.z].iter().all(|v| v.abs() <= f64::from(f32::MAX));
        let mesh = &source.mesh;
        let supported = !mesh.triangles.is_empty()
            && mesh.normals.len() == mesh.positions.len()
            && mesh.positions.iter().all(finite)
            && mesh.normals.iter().all(|normal| finite(normal) && normal.len() > 0.5)
            && mesh.triangles.iter().flatten().all(|index| (*index as usize) < mesh.positions.len());
        let tris = supported.then(|| Arc::new(SharedTris::new(mesh)));
        if self.entries.len() >= 256
            && let Some(oldest) = self.entries.iter().enumerate().min_by_key(|(_, entry)| entry.used).map(|(index, _)| index)
        {
            self.entries.swap_remove(oldest);
        }
        self.entries.push(CachedMesh { source: source.clone(), key: self.next_key, used: self.access, tris: tris.clone() });
        Ok(tris.map(|tris| (self.next_key, tris)))
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

pub(crate) fn model_geometry(snapshot: &RenderSnapshot, key: u64) -> Result<(GpuScene, ModelBodies), String> {
    model_geometry_cached(snapshot, key, &mut NativeMeshCache::default())
}

pub(crate) fn model_geometry_cached(snapshot: &RenderSnapshot, key: u64, cache: &mut NativeMeshCache) -> Result<(GpuScene, ModelBodies), String> {
    let source = &snapshot.model.scene;
    let names = snapshot.model.picks.body_names();
    let mut fallback = solidcraft_render::Scene::default();
    let mut fallback_names = Vec::new();
    let mut shared = Vec::new();
    // The snapshot retains every source Arc throughout this build, so pointer
    // identity is safe for this temporary grouping even beyond cache capacity.
    let mut current_meshes: std::collections::BTreeMap<usize, Option<(u64, Arc<SharedTris>)>> = std::collections::BTreeMap::new();
    for (index, (mesh, body)) in source.meshes.iter().zip(&names).enumerate() {
        let instance = snapshot.model.instances.get(index).filter(|instance| &instance.body == body);
        let rows =
            instance.map(|instance| std::array::from_fn::<_, 3, _>(|row| std::array::from_fn::<_, 4, _>(|col| instance.transform[col][row] as f32)));
        let eligible = !snapshot.model.occurrences.is_empty()
            && !mesh.glass()
            && mesh.face_colors.is_empty()
            && rows.as_ref().is_some_and(|rows| rows.iter().flatten().all(|v| v.is_finite()));
        if eligible && let (Some(instance), Some(rows)) = (instance, rows) {
            let identity = Arc::as_ptr(&instance.mesh) as usize;
            let converted = match current_meshes.get(&identity) {
                Some(converted) => converted.clone(),
                None => {
                    let converted = cache.shared(instance)?;
                    current_meshes.insert(identity, converted.clone());
                    converted
                }
            };
            if let Some((mesh_key, tris)) = converted {
                shared.push((body.clone(), mesh_key, tris, rows, [mesh.color.0, mesh.color.1, mesh.color.2, 255]));
                continue;
            }
        }
        fallback.meshes.push(solidcraft_render::SceneMesh {
            mesh: mesh.mesh.clone(),
            color: mesh.color,
            face_colors: mesh.face_colors.clone(),
            opacity: mesh.opacity,
        });
        fallback_names.push(body.clone());
    }
    // The public instancing API shares triangles. Existing world-space edges,
    // sketches and guides retain their established converter and appearance.
    let line = |line: &solidcraft_render::SceneLine| solidcraft_render::SceneLine {
        points: line.points.clone(),
        color: line.color,
        alpha: line.alpha,
        width: line.width,
        on_top: line.on_top,
    };
    fallback.lines = source.lines.iter().map(line).collect();
    fallback.grid = source.grid.iter().map(line).collect();
    let mut scene = solidcraft_gpu::capture::scene_geometry(&fallback);
    let mut bodies = fallback_blocks(&mut scene, &fallback, &fallback_names, key)?;
    let mut groups = std::collections::BTreeMap::new();
    for (body, mesh_key, tris, rows, rgba) in shared {
        let color = scene.color(rgba);
        let index = *groups.entry(mesh_key).or_insert_with(|| {
            let index = scene.instanced.len();
            scene.instanced.push(InstancedMesh {
                key: mesh_key,
                tris: tris.clone(),
                instances: Vec::new(),
                look: InstanceLook::Opaque,
                levels: Vec::new(),
            });
            index
        });
        if let Some(mesh) = scene.instanced.get_mut(index) {
            mesh.place(rows, color);
        }
        bodies.push((body, BodyGeometry::Instance { key: mesh_key, tris, rows }));
    }
    Ok((scene, bodies))
}

fn fallback_blocks(scene: &mut GpuScene, source: &solidcraft_render::Scene, names: &[String], key: u64) -> Result<ModelBodies, String> {
    let opaque = u32::try_from(scene.tris.len() / TRI_SIZE).map_err(|_| "model exceeds GPU vertex range")?;
    let glass = u32::try_from(scene.glass.len() / TRI_SIZE).map_err(|_| "model exceeds GPU vertex range")?;
    let total = opaque.checked_add(glass).ok_or("model exceeds GPU vertex range")?;
    let mut bytes = std::mem::take(&mut scene.tris);
    bytes.append(&mut scene.glass);
    let (mut solid_at, mut glass_at) = (0u32, opaque);
    let mut spans = Vec::new();
    let mut bodies = Vec::new();
    for (mesh, body) in source.meshes.iter().zip(names) {
        let triangles = mesh.mesh.triangles.iter().filter(|t| mesh.mesh.tri(t).is_some()).count();
        let vertices = u32::try_from(triangles).ok().and_then(|n| n.checked_mul(3)).ok_or("body exceeds GPU vertex range")?;
        let at = if mesh.glass() { &mut glass_at } else { &mut solid_at };
        let end = at.checked_add(vertices).ok_or("body exceeds GPU vertex range")?;
        let range = *at..end;
        let bounds = mesh.mesh.bounds();
        if !range.is_empty() {
            spans.push(BodySpan { vertices: range.clone(), bounds: [bounds.min.to_f32(), bounds.max.to_f32()] });
        }
        bodies.push((body.clone(), BodyGeometry::Block(BlockTris { block: key, tris: vec![range.clone()], vertices: range })));
        *at = end;
    }
    if solid_at != opaque || glass_at != total {
        return Err("body ranges do not match the shared renderer's vertices".into());
    }
    spans.sort_by_key(|span| span.vertices.start);
    if !bytes.is_empty() {
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
    }
    Ok(bodies)
}

pub(crate) fn highlight_geometry(snapshot: &RenderSnapshot, bodies: &ModelBodies) -> GpuScene {
    let mut highlight = solidcraft_gpu::capture::scene_geometry(&snapshot.highlight);
    for (selected, names, tint) in
        [(true, &snapshot.selected_bodies, snapshot.selection_tint), (false, &snapshot.hovered_bodies, snapshot.hover_tint)]
    {
        let rgba = tint.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        let color = highlight.color([rgba[0], rgba[1], rgba[2], 255]);
        for (_, geometry) in bodies.iter().filter(|(body, _)| names.contains(body)) {
            match geometry {
                BodyGeometry::Block(block) => {
                    if selected {
                        highlight.selected_blocks.push(block.clone());
                    } else {
                        highlight.hover_blocks.push(block.clone());
                    }
                }
                BodyGeometry::Instance { key, tris, rows } => {
                    let mut mesh =
                        InstancedMesh { key: *key, tris: tris.clone(), instances: Vec::new(), look: InstanceLook::Opaque, levels: Vec::new() };
                    mesh.place(*rows, color);
                    if selected {
                        highlight.selected_instanced.push(mesh);
                    } else {
                        highlight.hover_instanced.push(mesh);
                    }
                }
            }
        }
    }
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
