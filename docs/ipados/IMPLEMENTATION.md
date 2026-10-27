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

Runtime validation still needs a signed physical iPad: initial frame, resizing,
multitasking, foreground/background transitions, teardown, memory pressure and
actual Pencil/finger separation. Compilation and a macOS GPU picture do not prove
UIKit surface or iPad input runtime behavior. CI execution on GitHub is separate
from local verification; no runner/account settings were changed.
The unsigned native app build and 13 targeted tests pass. Complete upstream CI
remains blocked by an inherited geometry Clippy failure; see HANDOFF before
approval or integration.

## Acceptance criteria for Step 3

1. Start from the tested Step 2 state and resolve outstanding gates in HANDOFF.
2. Run the native app on a physical iPad and capture a real screenshot/recording
   showing command-generated geometry through the native surface. Verify rotation,
   split view, zero-size, foreground/background and scene close/reopen behavior.
3. Route one authoritative navigation implementation into the shared Rust camera;
   fingers navigate, Pencil selects without accidental orbit. Keep source identity
   through coalesced/cancelled events and correct safe-area/pixel conversion.
4. Add real engine picking/selection and command-driven model update rendering,
   with revision/stale-result tests and visible controlled errors.
5. Keep evaluation off the main thread. Define cooperative preview cancellation
   without claiming kernel preemption; reject stale previews after edits/close.
6. Preserve platform isolation, existing command/document semantics, and desktop
   and wasm checks. Do not copy upstream viewport/CAD modules into the host.
