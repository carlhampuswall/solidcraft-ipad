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

## Ownership and contracts (Step 3)

- `crates/ipados/session`: `SessionAdapter` exclusively owns the authoritative
  `engine::Session`. Existing IDs/JSON reach `Session::execute`; selection needed
  by an engine action is synchronised through existing `select.set`. No second
  evaluator, solver, document format or modelling command registry exists.
- `Viewport` owns the one display camera, contact routing, selection and hover.
  It holds an `Arc<SceneSnapshot>` and does not own another engine session. The
  engine's existing stateless navigation implementation is publicly re-exported
  as `navigate_camera`; `view.navigate` delegates to exactly that function.
  Standard views use the shared `standard_view`/`Camera::set_view`; fitting uses
  shared camera bounds/math with native aspect correction. The old native camera
  slot was removed from `SessionAdapter`.
- Engine command/evaluation/tessellation/index construction run on
  `SolidCraft.engine`; typed input, navigation, picking and rendering run on
  `SolidCraft.viewport`. UIKit layout stays on main. They exchange immutable
  owned scene Arcs, never borrowed vertex storage. A camera/selection operation
  never evaluates the engine or uploads the complete model again.
- `crates/ipados/bridge`: ABI **2**, checked integer handles (maximum 8), bounded
  JSON copies and independently locked engine, viewport and renderer state.
  Registry access only clones an entry Arc. The three response lanes (engine,
  viewport and surface) cannot overwrite one another. Each lane is serialised
  through response copying by the host. No engine pointer/allocation crosses C.
- The host retains CAMetalLayer until queued renderer destruction/replacement
  completes. Only attach/resize/draw/suspend/destroy and their surface-response
  copies share the lifecycle render lock. Main-thread resign-active suspension
  drains submitted GPU work with a bounded wait and **never takes the engine
  evaluation lock**. Native navigation remains available during recomputation.
  Zero drawable sizes pause; foreground activation can recreate GPU resources
  while retaining the engine and scene. Retry counts are bounded.
- `SceneSnapshot` contains the shared scene, visible world-space meshes indexed
  by actual body name, `MeshIndex` trees, world bounds, document ID, engine view
  revision and adapter geometry revision. The scene and picker are constructed
  from the same evaluated state. B-rep faces come from `Mesh::tri_face`, edges
  from `Mesh::edges`, vertices from the engine edges' endpoints. Display-only
  mesh outlines are excluded from edge/vertex selection. World transforms and
  visibility come from `Session::world_state` and `view::scene_with_guides`.
- `RenderSnapshot` retains the scene and the display camera/projection, guides
  and highlight scenes. The GPU model upload is keyed by document ID and
  geometry revision. Body tint references its existing GPU block; independent
  guide/highlight keys change only when their actual geometry changes.
  Clipping uses shared `Camera::view_proj_bounds`; the shared renderer handles
  forward/reversed GPU depth conversion. No idle render timer is installed;
  input, resize, command completion and bounded drawable retries request frames.
- Publication has a request generation, document identity and geometry revision.
  Old generations and callbacks after close are discarded. The previous scene
  stays visible during computation/failure; picking is disabled until a current
  scene publishes, and errors are visible. Kernel calls have no safe preemptive
  cancellation facility: queued work can be invalidated, not forcibly interrupted.
- Selection is a bounded list of real engine `Sel` values. The viewport owns
  display selection and mirrors it into the authoritative engine before CAD
  commands. Recompute, visibility changes, deletion, undo/redo and document
  replacement conservatively clear it; no persistent topology guarantee is added.
  Input carries the last **presented** geometry revision, so a tap on the old
  display cannot select a new mesh. Auto priority is vertex, edge, face; explicit
  body/face/edge/vertex filters resolve ambiguity. Edge/vertex tolerance is eight
  display points; candidate-position raycasts reject occluded geometry.
- `InputEvent` preserves `pencil`, `finger`, `indirect` or `unknown`, touch ID,
  phase, time, points/pixels/scale, pressure/altitude, button/modifier and consumed
  state. UIKit provides canvas-local points (origin zero); `ViewportMetrics`
  supports an explicit origin for other embeddings. The safe-area canvas and
  toolbar are separate hit-test hierarchies. Basic contact arbitration lives in
  `input.rs`; Step 4 can extend it without camera logic in Swift. Pointer hover
  is wired; Pencil hover, palm rejection and advanced simultaneous tools are not.
- Native document/file browser, coordinated security-scoped access, persistence,
  restoration and autosave remain future work. The generic command adapter is not
  restricted to the development fixture, and native-format engine commands are
  retained. No new native file format is introduced.

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
| `assets/locales/pending/ipados-viewport.json` | Native navigation/selection controls use the shared pending-catalog rule. | Namespaced strings with offline English fallback; no generated catalog changes. |
| `.github/workflows/ipados-build.yml` | A native SDK build needs a hosted macOS job. | Scoped/manual workflow, no Blacksmith, secrets or administration changes. Can accompany a future upstream host. |
| `docs/ipados/SETUP.md`, platform READMEs | Retire historical “reserved directory” statements and route to the implemented boundaries. | Documentation only; fork-specific. |

Step 3 additionally re-exports the existing stateless navigation implementation
(`crates/engine/src/{lib.rs,cmd/mod.rs,cmd/view_navigation.rs}`), without changing
its command contract or behavior. This is a small public, Apple-independent seam:
upstream navigation fixes still flow through one implementation; expected update
cost is checking the export and call signature. `crates/gpu/src/capture.rs`
adds `Capturer::rgba_viewport`: an optional prepared-model/feedback capture path
needed to verify the native adapter's actual GPU block references without copying
the renderer or readback pipeline. Existing capture methods delegate with their
original defaults; no Apple or host dependency is added. Expected update cost is
checking this small additive API and its wrapper on upstream capture changes.
No geometry, kernel, sketch, desktop/web frontend or dependency version changes.
After successful upstream integration, engine improvements
flow through these live dependencies. New operations may still need iPad UI and
input work; this does not promise automatic frontend parity.
