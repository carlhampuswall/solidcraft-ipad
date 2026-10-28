# Step 3 implementation status

The native viewport uses the real SolidCraft command engine, evaluator, world
state, B-rep tessellation and GPU viewport renderer. It now supports:

- Shaded bodies, actual model edges, visible sketch curves, an adaptive reference
  grid and a corner orientation triad; shared visibility and world transforms.
- One Rust camera path: orbit, pan, pinch/wheel zoom, fit, seven standard views,
  perspective and orthographic projection. Aspect, scale, offsets, zero-size,
  off-origin, small and large bounds are handled without modelling undo entries.
- Typed basic finger navigation, Pencil tap, pointer tap/hover, secondary drag
  orbit, middle/Shift-secondary drag pan and pointer scroll zoom. Toolbars consume
  their own touches. Detailed Pencil/finger arbitration remains Step 4.
- Body/face/edge/vertex filters, Auto priority, multi-select/toggle, clear,
  geometry-matched feedback and a selected type/body/index-or-point diagnostic.
- A repeatable command-generated fixture: a base, a separate raised cylindrical
  post and a reference sketch. The “Change post” control edits the real feature;
  undo/redo rebuild the scene. Generic engine requests support other designs.
- Separate engine and interactive workers; immutable scene/index publication;
  stale generation/presented-revision rejection; previous-scene retention on
  recomputation/error; current-pick gating and controlled visible errors.
- Stable model uploads with separate small guide/highlight resources, on-demand
  redraw, bounded lifecycle/GPU recovery and explicit session teardown.

Source/interface map and verification are in [HANDOFF.md](HANDOFF.md).
[ARCHITECTURE.md](ARCHITECTURE.md) records the small shared navigation export and
threading correction. [BUILDING.md](BUILDING.md) retains the device-only build path.

Step 3 has 28 passing focused tests, passing affected Clippy, adapter wasm and
web-offline checks, unsigned/signed native builds and physical-iPad launch/render.
The inherited full CI Clippy blocker remains documented in the handoff. The
final body-tint/scene-layout corrections have Mac GPU/build evidence; the iPad
locked before those final changes could be relaunched.
Compilation, synthetic workflows, physical touch and actual Pencil checks are
distinct; actual Pencil remains unverified. There is no frame-rate claim and no
simulator was used.

Remaining scope: advanced simultaneous Pencil/touch tools, palm handling, Pencil
hover/double-tap, sketch editing, polished UI, document browser and restoration.
Do not start those features before Step 4.

---

## Preserved Step 2 implementation record

The following records the foundation and the earlier physical-device follow-up;
its Step 3 acceptance list was the input to this milestone, not current status.

# Step 2 implementation status

The foundation consists of a real UIKit application target, Rust session adapter,
C ABI and shared GPU renderer integration. Architecture and ownership are in
[ARCHITECTURE.md](ARCHITECTURE.md); reproducible commands are in
[BUILDING.md](BUILDING.md). Exact observed results are in [HANDOFF.md](HANDOFF.md).

Implemented:

- iPad-only native entry point, scene lifecycle, safe-area Metal canvas, resizing,
  background work gating, bounded GPU suspension and explicit teardown.
- Safe Rust ownership of a real engine session; checked opaque handles; JSON
  commands/errors/revisions; owned immutable scene snapshots.
- Deterministic real `solid.box` command (40 × 30 × 10 mm), cache-free timeline
  rebuild, measured volume and engine tessellation. No handwritten proof mesh.
- Existing `solidcraft-gpu::view::Renderer` on a wgpu Metal surface; shared scene,
  palette and camera. Separate host-GPU proof uses the same snapshot and shared
  capture/viewport pipelines and reports actual draw statistics.
- Typed Pencil/finger input delivery and coordinate validation. Acknowledgement
  deliberately says `handled:false`; there is no selection/gesture implementation.
- Tests for engine lifetime, commands, hostile inputs, snapshot lifetime and ABI
  copies; resolved dependency isolation guard; scoped hosted macOS workflow.
- Native language and unsafe boundary exceptions documented and scoped.

Not implemented: interactive picking, orbit/pan/zoom gestures, sketch editing,
Pencil tools, document browser, open/save/autosave, restoration, assemblies or
polished CAD chrome. Offline evaluation/rendering is local, but durable document
workflows and interruption recovery are future integration work.

Physical-device follow-up: the signed app was installed and launched on an iPad
Pro; a device screenshot confirms the ready status and real geometry rendered
through the native Metal surface. Resizing, multitasking, foreground/background
transitions, teardown, memory pressure and actual Pencil/finger separation still
need runtime validation. CI execution on GitHub remains separate; no runner or
account settings were changed.
The unsigned native app build and 13 targeted tests pass. Complete upstream CI
remains blocked by an inherited geometry Clippy failure; see HANDOFF before
approval or integration.

## Acceptance criteria for Step 3

1. Start from the tested Step 2 state and resolve outstanding gates in HANDOFF.
2. Extend the confirmed physical-iPad launch/render proof with recordings of
   rotation, split view, zero-size, foreground/background and scene close/reopen
   behavior.
3. Route one authoritative navigation implementation into the shared Rust camera;
   fingers navigate, Pencil selects without accidental orbit. Keep source identity
   through coalesced/cancelled events and correct safe-area/pixel conversion.
4. Add real engine picking/selection and command-driven model update rendering,
   with revision/stale-result tests and visible controlled errors.
5. Keep evaluation off the main thread. Define cooperative preview cancellation
   without claiming kernel preemption; reject stale previews after edits/close.
6. Preserve platform isolation, existing command/document semantics, and desktop
   and wasm checks. Do not copy upstream viewport/CAD modules into the host.
