//! The only unsafe module: checked C buffers and the host-retained Apple layer.
//! Pointer validity cannot be inferred from an address; callers must uphold the
//! header's lifetime and buffer contracts. Engine ownership uses checked IDs.
#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{LazyLock, Mutex};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AdapterError, RenderSnapshot, SessionAdapter, renderer::NativeRenderer};

const MAX_REQUEST: usize = 1 << 20;
const MAX_RESPONSE: usize = 4 << 20;
const MAX_SESSIONS: usize = 8;

struct Entry {
    renderer: Option<NativeRenderer>,
    snapshot: Option<RenderSnapshot>,
    adapter: SessionAdapter,
    response: Vec<u8>,
    failed: bool,
}

struct Registry {
    next: u64,
    entries: BTreeMap<u64, Entry>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry { next: 1, entries: BTreeMap::new() }));

fn error(code: &'static str, message: impl Into<String>) -> AdapterError {
    AdapterError { code, message: message.into() }
}

fn with_entry(id: u64, operation: impl FnOnce(&mut Entry) -> Result<Value, AdapterError>) -> i32 {
    with_entry_mode(id, false, operation)
}

fn with_entry_mode(id: u64, cleanup: bool, operation: impl FnOnce(&mut Entry) -> Result<Value, AdapterError>) -> i32 {
    let Ok(mut registry) = REGISTRY.lock() else { return -2 };
    let Some(entry) = registry.entries.get_mut(&id) else { return -2 };
    let result = if entry.failed && !cleanup {
        Err(error("session_failed", "destroy and recreate the session after an internal failure"))
    } else {
        match catch_unwind(AssertUnwindSafe(|| operation(entry))) {
            Ok(result) => result,
            Err(_) => {
                entry.failed = true;
                Err(error("internal", "the native boundary caught an internal failure"))
            }
        }
    };
    let status = if result.is_ok() { 0 } else { -1 };
    let reply = match result {
        Ok(value) => json!({"ok": true, "result": value}),
        Err(error) => json!({"ok": false, "error": error}),
    };
    entry.response = serde_json::to_vec(&reply).unwrap_or_default();
    if entry.response.len() > MAX_RESPONSE {
        entry.response = br#"{"ok":false,"error":{"code":"response_limit","message":"response exceeds 4 MiB"}}"#.to_vec();
        return -1;
    }
    status
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Proof,
    Command { command: String, params: Value },
    Snapshot,
    Input { event: InputEvent },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum InputSource {
    Pencil,
    Finger,
    Indirect,
    Unknown,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum InputPhase {
    Began,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputEvent {
    source: InputSource,
    phase: InputPhase,
    id: u64,
    x_points: f64,
    y_points: f64,
    x_pixels: f64,
    y_pixels: f64,
    scale: f64,
    timestamp: f64,
    pressure: Option<f64>,
    altitude: Option<f64>,
}

fn input(event: InputEvent) -> Result<Value, AdapterError> {
    let InputEvent { source, phase, id, x_points, y_points, x_pixels, y_pixels, scale, timestamp, pressure, altitude } = event;
    if !([x_points, y_points, x_pixels, y_pixels, scale, timestamp].iter().all(|v| v.is_finite()) && 0.0 < scale && scale <= 8.0)
        || timestamp < 0.0
        || pressure.is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
        || altitude.is_some_and(|a| !a.is_finite() || !(0.0..=std::f64::consts::FRAC_PI_2).contains(&a))
        || (x_points * scale - x_pixels).abs() > 0.01
        || (y_points * scale - y_pixels).abs() > 0.01
    {
        return Err(error("input", "invalid input coordinates or device values"));
    }
    let source = match source {
        InputSource::Pencil => "pencil",
        InputSource::Finger => "finger",
        InputSource::Indirect => "indirect",
        InputSource::Unknown => "unknown",
    };
    let phase = match phase {
        InputPhase::Began => "began",
        InputPhase::Moved => "moved",
        InputPhase::Ended => "ended",
        InputPhase::Cancelled => "cancelled",
    };
    // Step 2 transports identity and coordinates only. Step 3 supplies one shared navigation path.
    Ok(json!({"source":source,"phase":phase,"id":id,"x_pixels":x_pixels,"y_pixels":y_pixels,"handled":false}))
}

fn metadata(snapshot: &RenderSnapshot) -> Value {
    json!({"document_id":snapshot.document_id,"revision":snapshot.revision,"geometry_revision":snapshot.geometry_revision,"scene_revision":snapshot.scene_revision,
        "body_count":snapshot.body_count,"triangle_count":snapshot.triangle_count,"volume_mm3":snapshot.volume_mm3})
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
        registry.entries.insert(id, Entry { renderer: None, snapshot: None, adapter: SessionAdapter::new(), response: Vec::new(), failed: false });
        id
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_session_destroy(id: u64) -> i32 {
    catch_unwind(|| {
        let Ok(mut registry) = REGISTRY.lock() else { return -2 };
        match registry.entries.remove(&id) {
            Some(mut entry) => {
                entry.renderer.take();
                drop(entry);
                0
            }
            None => -2,
        }
    })
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_request(id: u64, bytes: *const u8, length: usize) -> i32 {
    with_entry(id, |entry| {
        if bytes.is_null() || length == 0 || length > MAX_REQUEST {
            return Err(error("request", "request must contain 1 byte to 1 MiB of JSON"));
        }
        // SAFETY: non-null and bounded above; the host guarantees readable memory for length.
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        let request: Request = serde_json::from_slice(bytes).map_err(|e| error("request", e.to_string()))?;
        match request {
            Request::Input { event } => input(event),
            Request::Proof => {
                entry.snapshot = None;
                let snapshot = entry.adapter.proof()?;
                let reply = metadata(&snapshot);
                entry.snapshot = Some(snapshot);
                Ok(reply)
            }
            Request::Command { command, params } => {
                entry.snapshot = None;
                let reply = entry.adapter.execute(&command, params)?;
                // Invalidate first: a failed scene rebuild must never display old geometry as current.
                entry.snapshot = Some(entry.adapter.snapshot()?);
                serde_json::to_value(reply).map_err(|e| error("response", e.to_string()))
            }
            Request::Snapshot => {
                let snapshot = entry.adapter.snapshot()?;
                let reply = metadata(&snapshot);
                entry.snapshot = Some(snapshot);
                Ok(reply)
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_response_len(id: u64) -> usize {
    REGISTRY.lock().ok().and_then(|registry| registry.entries.get(&id).map(|e| e.response.len())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_response_copy(id: u64, output: *mut u8, capacity: usize) -> i32 {
    let Ok(registry) = REGISTRY.lock() else { return -2 };
    let Some(entry) = registry.entries.get(&id) else { return -2 };
    if output.is_null() || capacity < entry.response.len() {
        return -1;
    }
    // SAFETY: host guarantees writable, nonoverlapping memory for capacity, checked above.
    unsafe {
        std::ptr::copy_nonoverlapping(entry.response.as_ptr(), output, entry.response.len());
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sc_surface_attach(id: u64, layer: *mut c_void, width: u32, height: u32) -> i32 {
    with_entry(id, |entry| {
        if layer.is_null() {
            return Err(error("surface", "the host layer is null"));
        }
        #[cfg(target_vendor = "apple")]
        {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::METAL;
            let instance = wgpu::Instance::new(descriptor);
            // SAFETY: the native host retains this live CAMetalLayer until renderer destruction.
            // No borrowed session data crosses this boundary; the surface owns its wgpu resources.
            let surface = unsafe { instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer)) }
                .map_err(|e| error("surface", e.to_string()))?;
            let renderer = NativeRenderer::new(&instance, surface, width, height).map_err(|e| error("renderer", e))?;
            entry.renderer = Some(renderer);
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
    with_entry(id, |entry| {
        let renderer = entry.renderer.as_mut().ok_or_else(|| error("surface", "no surface is attached"))?;
        renderer.set_active(true).map_err(|e| error("renderer", e))?;
        renderer.resize(width, height).map_err(|e| error("renderer", e))?;
        Ok(json!({"width":width,"height":height}))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_surface_draw(id: u64) -> i32 {
    with_entry(id, |entry| {
        let snapshot = entry.snapshot.as_ref().ok_or_else(|| error("geometry", "no evaluated scene is available"))?;
        let renderer = entry.renderer.as_mut().ok_or_else(|| error("surface", "no surface is attached"))?;
        let outcome = renderer.draw(snapshot).map_err(|e| error("renderer", e))?;
        Ok(json!({"geometry_revision":snapshot.geometry_revision,"triangle_count":snapshot.triangle_count,
            "drawn_triangles":outcome.stats.triangles,"presented":outcome.presented}))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn sc_surface_suspend(id: u64) -> i32 {
    with_entry_mode(id, true, |entry| {
        if let Some(renderer) = entry.renderer.as_mut() {
            renderer.set_active(false).map_err(|e| error("renderer", e))?;
        }
        Ok(json!({"active":false}))
    })
}

#[cfg(test)]
mod tests;
