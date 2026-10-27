# Native iPadOS foundation

Decision (Step 2): a thin UIKit host, a safe Rust session adapter, and SolidCraft's
existing Rust GPU renderer on a native CAMetalLayer. Minimum deployment: iPadOS 17.
Everything needed to create/evaluate/display the proof is local; no WebView,
network engine, copied CAD modules or second kernel is involved.

## Evidence and tradeoffs

The lockfile contains egui/eframe/egui-winit 0.36.2, winit 0.30.13, wgpu 30.0.1,
rfd 0.17.2 and arboard 3.6.1. Eframe and egui-winit have iOS paths; Metal support
alone does not make SolidCraft's desktop application an iOS host.

| Approach | Findings at this revision |
| --- | --- |
| Existing Rust/egui frontend | Reuses the most UI, but desktop `apps/solidcraft` enables X11 on non-macOS Unix and invokes rfd. `cargo check -p rfd --target aarch64-apple-ios` fails with 12 missing dialog-backend trait implementations. `ui-egui` unconditionally includes arboard on native targets; its iOS check succeeds but selects its X11 Unix backend. Winit Touch/egui-winit's mapping does not preserve UIKit Pencil versus finger identity. A viable port needs shared UI/service gates and native touch integration. |
| Thin UIKit host (chosen) | Preserves Pencil identity directly, owns native lifecycle/files/safe areas and avoids shared frontend changes. Reuses the actual engine, evaluator, scene builder, palette, camera and GPU pipelines. Costs a small C ABI and eventual native UI forms; modelling semantics must remain Rust commands. |

The compile spike is intentionally limited to the locked dialog dependency; two
complete frontends were not built. Upstream draft [iOS port #7456](https://github.com/tangency-labs/solidcraft/pull/7456)
was open during inspection, describing touch-selection/navigation limitations.
This is a fork foundation, not a competing upstream claim; no code was imported
from that draft. Reassess shared frontend viability when integrating its outcome.

## Ownership and contracts

- `crates/ipados/session`: safe L5 adapter owns one `engine::Session` and shared
  `render::Camera`. Existing command IDs/JSON go unchanged to `Session::execute`.
  `AdapterError {code,message}` becomes a JSON error, never a dialog.
- `crates/ipados/bridge`: L6 host boundary, checked integer handles (maximum 8 live
  sessions), a process mutex, bounded byte copies and the surface renderer. The
  header `include/solidcraft_ipados.h` is ABI 1. No Rust allocation or engine
  pointer crosses it. The host explicitly destroys each handle.
- `apps/solidcraft-ipados`: UIKit owns scenes, views and the CAMetalLayer; one
  serial worker owns session work. Commands, rebuilds, tessellation,
  pipeline initialization and rendering run off the window thread. The host
  retains the layer until queued Rust renderer destruction completes. UIKit view
  creation/layout remains on main; wgpu's direct layer backend supports worker
  rendering. Resize uses drawable pixels, zero sizes pause, and lifecycle changes
  gate queued work. A render-only lock serializes surface calls and response
  copies with synchronous resign-active suspension, which drains submitted GPU
  work with a bounded wait. Startup CAD proof runs before a surface exists and
  outside that lock. Before adding long CAD jobs after surface attachment, split
  the current registry/engine lock from render suspension so the main thread
  cannot wait on evaluation; Step 2's host submits only the startup CAD operation.
- Replies include `document_id`, document `revision`, conservative
  `geometry_revision`, `scene_revision`, and `changed`. The scene token also changes
  for view commands and failed attempts; it prevents reuse of stale coloured or
  camera-dependent scene data when the document revision stays unchanged. They are
  the current notification mechanism;
  there is no second observer framework. Command schema coverage is incomplete
  upstream and cannot be assumed permanently stable.
- `RenderSnapshot` owns the shared `Scene` (retaining mesh Arcs) and copies the
  shared camera. It can outlive the mutable engine. No borrowed vertex memory is
  handed to Swift. GPU upload keys include document identity and scene revision.
- Work is serial today. There is no preemptive cancellation inside kernel calls.
  Lifecycle/close invalidates queued work; destruction waits its turn. Future
  asynchronous previews must capture document identity, revision and request
  generation, and discard stale results before presentation. Do not claim an
  arbitrary engine command can currently be interrupted safely.
- Input JSON preserves `pencil`, `finger`, `indirect`, or `unknown`, stable touch
  ID, phase, timestamp, force/altitude and view-local points plus pixels. Origin
  is top-left inside the safe-area canvas; `pixels = points × contentScaleFactor`.
  Validation checks finite numbers and conversion. Step 2 acknowledges input with
  `handled:false`; it implements no gestures. Pencil will select/sketch/manipulate;
  fingers will navigate and intentionally operate UI.
- There is one Rust camera. Future navigation must adapt to the shared
  `navigation::handle`/camera semantics through one authoritative path. The existing
  `view.navigate` engine command is a suitable native entry point. Do not add
  a Swift camera or independent orbit controller. The current proof only fits
  that shared camera to the real scene.
- Future native document access: the host obtains security-scoped URLs and
  coordinates access; Rust uses existing file commands/formats and atomic writes.
  URLs/bookmarks remain host-owned and scopes remain open until worker completion.
  File picking, persistence, restoration and autosave are not implemented here.
  The in-memory proof works offline; durable document workflows need later work.

## Approved platform exceptions

The Step 2 request explicitly permits a minimal non-Rust Apple host and narrow
FFI. Swift is confined to the native app; no modelling or document logic moves
there. UIKit system colours and safe-area containers supply native chrome; no
parallel egui container is constructed. Diagnostic status strings use the pending
locale catalogue bundled with the app.

Only `bridge/src/ffi.rs` allows unsafe: C exports, validated-length input/output
buffer operations and `Instance::create_surface_unsafe(CoreAnimationLayer)`.
The header states pointer/layer validity and lifetime preconditions. Callers are
the bundled trusted host, not arbitrary native plugins. Pointer provenance cannot
be checked from an address. The bridge root denies unsafe, the adapter forbids it,
and shared Rust safety rules are unchanged. Panics at the boundary become errors;
an internally failed handle must be destroyed and recreated.

## Upstream delta ledger

| File outside platform directories | Why adapter alone is insufficient; minimal change | Desktop/web impact; upstream suitability |
| --- | --- | --- |
| `Cargo.toml` | Register two actual nested packages; retain native host/namespace exclusions. | Adds members only; no changed shared dependencies. Fork-specific registration. |
| `Cargo.lock` | Cargo records the two local packages. | No external version changes. Fork-specific. |
| `xtask/src/layers.rs` | The architecture gate rejects unregistered packages; classify adapter L5 and bridge L6. | Existing layer restrictions remain; bridge also participates in wasm checks. Fork-specific registrations. |
| `crates/doc/src/walk.rs` | Rust 1.95 warns about the generic container macro iterating Option, blocking `-D warnings`; an adapter cannot repair a dependency lint. Use six explicit `iter`/`iter_mut` calls. | Same elements, lifetimes and traversal order on Option/Vec/arrays; no CAD behavior change. Suitable as a separate upstream lint cleanup. |
| `AGENTS.md`, `skills/solidcraft-crash-safety/SKILL.md` | Explicitly locate the approved native language/unsafe exception. | Shared rules retained. Fork policy, not an upstream safety relaxation. |
| `assets/locales/pending/ipados-foundation.json` | Status messages follow the shared pending-catalog rule. | Adds namespaced strings only; generated catalogs untouched. Can accompany a future upstream host. |
| `.github/workflows/ipados-build.yml` | A native SDK build needs a hosted macOS job. | Scoped/manual workflow, no Blacksmith, secrets or administration changes. Can accompany a future upstream host. |
| `docs/ipados/SETUP.md`, platform READMEs | Retire historical “reserved directory” statements and route to the implemented boundaries. | Documentation only; fork-specific. |

The only shared CAD source change is the container iteration lint cleanup above;
no engine, kernel, sketch, render, GPU or desktop/web application logic changes.
After successful upstream integration, engine improvements
flow through these live dependencies. New operations may still need iPad UI and
input work; this does not promise automatic frontend parity.
