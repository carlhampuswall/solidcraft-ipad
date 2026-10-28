//! The sole unsafe boundary: checked C buffers and host-retained CAMetalLayer.
//! The registry is held only to clone an entry. Evaluation, display state, renderer
//! and their response lanes have independent locks. Suspension never locks CAD.
#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use serde::Deserialize;
use serde_json::{Value, json};
use solidcraft_ipados_session::{InputEvent, SceneSnapshot, Viewport, ViewportMetrics};

use crate::{AdapterError, SessionAdapter, renderer::NativeRenderer};

const MAX_REQUEST: usize = 1 << 20;
const MAX_RESPONSE: usize = 4 << 20;
const MAX_SESSIONS: usize = 8;

struct Entry {
    renderer: Mutex<Option<NativeRenderer>>,
    viewport: Mutex<Viewport>,
    adapter: Mutex<SessionAdapter>,
    responses: [Mutex<Vec<u8>>; 3],
    generation: AtomicU64,
    failed: AtomicBool,
    closed: AtomicBool,
}

struct Registry {
    next: u64,
    entries: BTreeMap<u64, Arc<Entry>>,
}
static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry { next: 1, entries: BTreeMap::new() }));

fn error(code: &'static str, message: impl Into<String>) -> AdapterError {
    AdapterError { code, message: message.into() }
}
fn lookup(id: u64) -> Option<Arc<Entry>> {
    REGISTRY.lock().ok()?.entries.get(&id).cloned()
}

// 0 engine, 1 viewport/input, 2 renderer/lifecycle. Each lane is serialised by the
// host through its response copy, but different lanes may execute concurrently.
fn with_entry(id: u64, lane: usize, cleanup: bool, operation: impl FnOnce(&Entry) -> Result<Value, AdapterError>) -> i32 {
    let Some(entry) = lookup(id) else { return -2 };
    let result = if entry.closed.load(Ordering::Acquire) {
        Err(error("closed", "session closed"))
    } else if entry.failed.load(Ordering::Acquire) && !cleanup {
        Err(error("session_failed", "destroy and recreate the session after an internal failure"))
    } else {
        match catch_unwind(AssertUnwindSafe(|| operation(&entry))) {
            Ok(result) => result,
            Err(_) => {
                entry.failed.store(true, Ordering::Release);
                Err(error("internal", "the native boundary caught an internal failure"))
            }
        }
    };
    let status = if result.is_ok() { 0 } else { -1 };
    let reply = match result {
        Ok(value) => json!({"ok":true,"result":value}),
        Err(error) => json!({"ok":false,"error":error}),
    };
    let bytes = serde_json::to_vec(&reply).unwrap_or_default();
    let Some(response) = entry.responses.get(lane) else { return -2 };
    let Ok(mut response) = response.lock() else { return -2 };
    if bytes.len() > MAX_RESPONSE {
        *response = br#"{"ok":false,"error":{"code":"response_limit","message":"response exceeds 4 MiB"}}"#.to_vec();
        return -1;
    }
    *response = bytes;
    status
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Proof,
    Fixture,
    Command { command: String, params: Value },
    Snapshot,
    Input { event: InputEvent },
    Viewport { metrics: ViewportMetrics },
}

impl Entry {
    fn display_request(&self, request: Request) -> Result<Value, AdapterError> {
        let mut viewport = self.viewport.lock().map_err(|_| error("lock", "viewport lock poisoned"))?;
        match request {
            Request::Input { event } => viewport.input(event),
            Request::Viewport { metrics } => {
                viewport.set_metrics(metrics)?;
                Ok(viewport.diagnostic())
            }
            Request::Command { command, params } => viewport.command(&command, &params),
            Request::Snapshot => Ok(viewport.diagnostic()),
            _ => Err(error("request", "evaluation requests belong on the engine lane")),
        }
    }

    fn evaluate(&self, request: Request) -> Result<Value, AdapterError> {
        if matches!(&request, Request::Input { .. } | Request::Viewport { .. })
            || matches!(&request, Request::Command { command, .. } if command.starts_with("view.") || command.starts_with("select."))
        {
            return self.display_request(request);
        }
        let generation = self
            .generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .map_err(|_| error("revision_limit", "request generation exhausted"))?
            + 1;
        let selection = {
            let mut viewport = self.viewport.lock().map_err(|_| error("lock", "viewport lock poisoned"))?;
            let selection = viewport.selection().to_vec();
            viewport.begin_update(generation);
            selection
        };
        let result = (|| {
            let mut adapter = self.adapter.lock().map_err(|_| error("lock", "engine lock poisoned"))?;
            match request {
                Request::Proof => adapter.proof().map(|s| (metadata(&s), s)),
                Request::Fixture => adapter.fixture().map(|s| (metadata(&s), s)),
                Request::Command { command, params } => {
                    adapter.selection(&selection)?;
                    let reply = adapter.execute(&command, params)?;
                    // Read-only commands keep the previous geometry/index without re-tessellating.
                    let existing = self.viewport.lock().map_err(|_| error("lock", "viewport lock poisoned"))?.snapshot();
                    let snapshot =
                        match existing.filter(|s| s.model.document_id == reply.document_id && s.geometry_revision == reply.geometry_revision) {
                            Some(snapshot) => snapshot.model,
                            None => adapter.snapshot()?,
                        };
                    let reply = serde_json::to_value(reply).map_err(|e| error("response", e.to_string()))?;
                    Ok((reply, snapshot))
                }
                Request::Snapshot => adapter.snapshot().map(|s| (metadata(&s), s)),
                _ => Err(error("request", "invalid engine request")),
            }
        })();
        // Bad parameters/unknown commands leave a valid engine document. Recover its
        // current scene on this worker so an ordinary command error cannot strand picking.
        let recovered = if result.is_err() { self.adapter.lock().ok().and_then(|mut adapter| adapter.snapshot().ok()) } else { None };
        let mut viewport = self.viewport.lock().map_err(|_| error("lock", "viewport lock poisoned"))?;
        match result {
            Ok((reply, snapshot)) => {
                if self.closed.load(Ordering::Acquire) || !viewport.install(generation, snapshot) {
                    return Err(error("stale_result", "discarded an outdated evaluation result"));
                }
                Ok(json!({"command":reply,"viewport":viewport.diagnostic()}))
            }
            Err(failure) => {
                if let Some(scene) = recovered.filter(|_| !self.closed.load(Ordering::Acquire)) {
                    viewport.install(generation, scene);
                } else {
                    viewport.fail_update(generation);
                }
                Err(failure)
            }
        }
    }
}

fn metadata(s: &SceneSnapshot) -> Value {
    json!({"document_id":s.document_id,"revision":s.revision,"geometry_revision":s.geometry_revision,
        "body_count":s.body_count,"triangle_count":s.triangle_count,"volume_mm3":s.volume_mm3})
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_session_create() -> u64 {
    catch_unwind(|| {
        let Ok(mut registry) = REGISTRY.lock() else { return 0 };
        if registry.entries.len() >= MAX_SESSIONS {
            return 0;
        }
        let id = registry.next;
        let Some(next) = id.checked_add(1) else { return 0 };
        registry.next = next;
        registry.entries.insert(
            id,
            Arc::new(Entry {
                renderer: Mutex::new(None),
                viewport: Mutex::new(Viewport::default()),
                adapter: Mutex::new(SessionAdapter::new()),
                responses: std::array::from_fn(|_| Mutex::new(Vec::new())),
                generation: AtomicU64::new(0),
                failed: AtomicBool::new(false),
                closed: AtomicBool::new(false),
            }),
        );
        id
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_session_destroy(id: u64) -> i32 {
    catch_unwind(|| {
        let entry = {
            let Ok(mut r) = REGISTRY.lock() else { return -2 };
            r.entries.remove(&id)
        };
        let Some(entry) = entry else { return -2 };
        entry.closed.store(true, Ordering::Release);
        if let Ok(mut renderer) = entry.renderer.lock() {
            renderer.take();
        }
        0
    })
    .unwrap_or(-1)
}

unsafe fn request(id: u64, bytes: *const u8, length: usize, lane: usize) -> i32 {
    with_entry(id, lane, false, |entry| {
        if bytes.is_null() || length == 0 || length > MAX_REQUEST {
            return Err(error("request", "request must contain 1 byte to 1 MiB of JSON"));
        }
        // SAFETY: pointer/length bounded here; readable allocation retained by the host.
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        let request: Request = serde_json::from_slice(bytes).map_err(|e| error("request", e.to_string()))?;
        if lane == 1 { entry.display_request(request) } else { entry.evaluate(request) }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_request(id: u64, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: forwarded unchanged to the checked, bounded request reader.
    unsafe { request(id, bytes, length, 0) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_view_request(id: u64, bytes: *const u8, length: usize) -> i32 {
    // SAFETY: same host-owned buffer contract as sc_request.
    unsafe { request(id, bytes, length, 1) }
}

fn response_len(id: u64, lane: usize) -> usize {
    lookup(id).and_then(|e| e.responses.get(lane)?.lock().ok().map(|r| r.len())).unwrap_or(0)
}
unsafe fn response_copy(id: u64, output: *mut u8, capacity: usize, lane: usize) -> i32 {
    let Some(entry) = lookup(id) else { return -2 };
    let Some(response) = entry.responses.get(lane) else { return -2 };
    let Ok(response) = response.lock() else { return -2 };
    if output.is_null() || capacity < response.len() {
        return -1;
    }
    // SAFETY: host owns capacity writable, nonoverlapping bytes; size checked above.
    unsafe {
        std::ptr::copy_nonoverlapping(response.as_ptr(), output, response.len());
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_response_len(id: u64) -> usize {
    response_len(id, 0)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_response_copy(id: u64, output: *mut u8, capacity: usize) -> i32 {
    // SAFETY: host buffer contract as documented in the C header.
    unsafe { response_copy(id, output, capacity, 0) }
}
#[unsafe(no_mangle)]
pub extern "C" fn sc_view_response_len(id: u64) -> usize {
    response_len(id, 1)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_view_response_copy(id: u64, output: *mut u8, capacity: usize) -> i32 {
    // SAFETY: host buffer contract as documented in the C header.
    unsafe { response_copy(id, output, capacity, 1) }
}
#[unsafe(no_mangle)]
pub extern "C" fn sc_surface_response_len(id: u64) -> usize {
    response_len(id, 2)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_surface_response_copy(id: u64, output: *mut u8, capacity: usize) -> i32 {
    // SAFETY: host buffer contract as documented in the C header.
    unsafe { response_copy(id, output, capacity, 2) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_surface_attach(id: u64, layer: *mut c_void, width: u32, height: u32) -> i32 {
    with_entry(id, 2, false, |entry| {
        if layer.is_null() {
            return Err(error("surface", "the host layer is null"));
        }
        #[cfg(target_vendor = "apple")]
        {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::METAL;
            let instance = wgpu::Instance::new(descriptor);
            // SAFETY: the host retains this live CAMetalLayer until renderer destruction.
            let surface = unsafe { instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer)) }
                .map_err(|e| error("surface", e.to_string()))?;
            let renderer = NativeRenderer::new(&instance, surface, width, height).map_err(|e| error("renderer", e))?;
            *entry.renderer.lock().map_err(|_| error("lock", "renderer lock poisoned"))? = Some(renderer);
            Ok(json!({"surface":"metal","width":width,"height":height}))
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            let _ = (entry, width, height);
            Err(error("unsupported", "a CAMetalLayer requires an Apple target"))
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_surface_resize(id: u64, width: u32, height: u32) -> i32 {
    with_entry(id, 2, false, |entry| {
        let mut renderer = entry.renderer.lock().map_err(|_| error("lock", "renderer lock poisoned"))?;
        let renderer = renderer.as_mut().ok_or_else(|| error("surface", "no surface is attached"))?;
        renderer.set_active(true).map_err(|e| error("renderer", e))?;
        renderer.resize(width, height).map_err(|e| error("renderer", e))?;
        Ok(json!({"width":width,"height":height}))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_surface_draw(id: u64) -> i32 {
    with_entry(id, 2, false, |entry| {
        let (snapshot, diagnostic) = {
            let viewport = entry.viewport.lock().map_err(|_| error("lock", "viewport lock poisoned"))?;
            let Some(snapshot) = viewport.snapshot() else {
                return Ok(json!({"presented":false,"viewport":viewport.diagnostic()}));
            };
            (snapshot, viewport.diagnostic())
        };
        let mut renderer = entry.renderer.lock().map_err(|_| error("lock", "renderer lock poisoned"))?;
        let renderer = renderer.as_mut().ok_or_else(|| error("surface", "no surface is attached"))?;
        let outcome = renderer.draw(&snapshot).map_err(|e| error("renderer", e))?;
        Ok(json!({"geometry_revision":snapshot.geometry_revision,"triangle_count":snapshot.model.triangle_count,
            "drawn_triangles":outcome.stats.triangles,"presented":outcome.presented,"viewport":diagnostic}))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_surface_suspend(id: u64) -> i32 {
    with_entry(id, 2, true, |entry| {
        if let Some(renderer) = entry.renderer.lock().map_err(|_| error("lock", "renderer lock poisoned"))?.as_mut() {
            renderer.set_active(false).map_err(|e| error("renderer", e))?;
        }
        Ok(json!({"active":false}))
    })
}

#[cfg(test)]
mod tests;
