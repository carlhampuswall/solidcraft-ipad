# Step 8.5 consolidated implementation status

Source baseline **B = `c22acd12702cf3a405df01009b8d8662b3d46365`** is committed on `integration/ipados-foundation`. It is accepted for development under the bounded 2026-10-28 user exception, not certified for release. Tasks A–D are prepared for individually requested launches; this task has not launched them or pushed the branch. Read finalized planning documents from the documentation-only descendant **P** identified in the integrator's final report; create every implementation worktree at B. P is never embedded as its own hash.

The completed full `cargo +1.95.0 xtask ci` run exits **1**: **85/86 top-level checks pass**; the engine library has **2631 passed, 3 failed, 6 ignored**. Only the three unchanged failures in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) remain under the user exception. All other required checks passed. **CI is not green.**

Steps 1–8 use one authoritative engine session/command/history boundary, central input ownership, shared camera/navigation, native transforms and scratch preview/atomic acceptance. Consolidation preserves their code and adds the suspension regression, seventeen equivalent compiler lint repairs, test-only optimized crash-fixture correction and native catalog-domain isolation. ARCHITECTURE records the shared maintenance surface; no new CAD feature or kernel algorithm was implemented in this task.

The baseline preserves the separate user-requested two-finger double-tap Undo and three-finger double-tap Redo work from “Add multitouch undo and redo” (`01a183dd-c2f4-74a1-a93d-024ef30a9e7c`). It uses existing engine history. `SolidCraft.workspace.historyGestures` defaults on and restores the earlier behavior when disabled. The separate UI overhaul task contributed no source changes.

The resumed combined unsigned arm64 Rust/UIKit build passes in `/tmp/solidcraft-step85-resume-native-build.log`; all **186 Foundation cases** pass in `/tmp/solidcraft-step85-resume-native-check.log`. The native English catalog contains **373 keys**. No physical app launch, simulator, installation or signing operation was performed in this consolidation; physical gesture checks remain outstanding.

Tests remain enabled; expectations, tolerances and goldens are unchanged. Any new failed test or changed failure signature blocks acceptance. Recheck the exception after a relevant upstream fix, affected engine/kernel/toolchain changes, and before release. No automatic monitor was created.

HANDOFF records exact current validation evidence. The milestone sections below are historical; their branch names, pending status, totals and earlier next-step instructions do not override the committed B and finalized policy P.

---

# Step 8 implementation status

8A, 8B and the supported 8C interactions extend the existing workspace with
native components, occurrence paths, placement and joints. See
[ASSEMBLIES.md](ASSEMBLIES.md) for the command/capability inventory and
[HANDOFF.md](HANDOFF.md) for actual validation, blockers and Step 9 seams.

- Components: native creation and body ownership transfer, explicit activation
  and return to root, linked occurrences versus independent Paste New, exact
  occurrence selection/highlighting, hierarchy, rename and visibility/isolation.
- Placement: world/parent-local translation, rotation about an explicit pivot,
  native grounding, scoped pending-position capture/revert, Pencil parameter
  handles and precise numeric fields. Grounded/jointed parts cannot bypass native
  refusal; constrained drag/reach and joint driving remain engine operations.
- Joints: rigid, revolute, slider, cylindrical, planar, pin-slot and ball;
  geometric/named/explicit frames, flip/offset, normal/as-built creation,
  original-ID edit/rebind, limits, suppression, confirmed deletion and actual
  solver feedback. Native DOF controls combine exact fields, sliders and handles.
- Safety/history: existing scratch/ComputedCommands previews, bounded latest-work
  scheduling, source/document/interaction checks, explicit Apply, native history
  and full native container save/reopen. Named references are validated before
  previews, with deliberate repair and no nearby-geometry substitution.
- Rendering: native source mesh instances and effective transforms feed existing
  SharedTris/InstancedMesh APIs through a bounded triangle cache. Picking and
  selected-instance feedback use the same evaluated revision/pose. World edge
  streams and material fallbacks remain; no measured performance claim.

New adapter modules are `assembly.rs` and `assembly_references.rs`. Existing
modelling/input/workspace/viewport/bridge/native files and area test files are
extended; no assembly solver, component graph, transform store or history stack
was added. Two additive shared command changes support scoped
`component.capture_position` and original-joint A/B rebinding in `joint.edit`.
Their rationale, compatibility and upstream maintenance are in the delta ledgers.
There are no format, dependency or shared kernel changes.

Deterministic acceptance passes: 105 session tests (23 assembly cases), 23 bridge,
163 shared-joint and 77 shared-occurrence tests, native Foundation cases, wasm,
web offline, desktop app all-targets check, unsigned native build, 11 inspected Mac GPU captures and rebuilt CLI
compatibility. Physical iPad controller runtime passes joint creation,
preview/cancel, commits, limits, Undo/Redo and native save/reopen with matching presented Metal frames.
The user verified physical occurrence selection, finger navigation and the
Pencil hinge preview/cancel workflow after the reproduced preview-flicker fix.
Palm/VoiceOver/performance and simultaneous hand-input checks remain outstanding.
Full CI still stops at the inherited geom Clippy error, preventing commits; nothing was
pushed and prior uncommitted milestones remain intact.

Native origin IDs cannot identify an independent placement of a repeated nested
definition: new ambiguous joint endpoints are rejected. Existing sharing remains
native. Motion-study/coupling/rigid-group editors and occurrence/component delete
UI remain unexposed; full native files/recovery is Step 9. No later milestone was
started.

## Retained Step 7 implementation record

# Step 7 implementation status

7A, 7B and 7C basic modelling workflows now run through the native workspace.
[MODELLING.md](MODELLING.md) maps exact commands, input/target semantics,
transactional previews, original-feature editing, reference repair and native
persistence. [HANDOFF.md](HANDOFF.md) records actual validation and limits.

Implemented: classified-profile extrusion (new/join/cut/intersect, signed or
symmetric distance, Through All), exact-fit fillet modes and chamfer controls,
shell, revolve, simple holes, all three Booleans, body/feature rectangular,
circular and path patterns, mirror, sweep and ordered-section loft. The real
feature timeline supports original-parameter edits, rename, suppression,
dependency inspection and explicit deletion. Sketch changes regenerate the
original feature chain. Named expressions and app-private native Save/Reopen
reuse upstream validation/serialization. Advanced options and physical runtime
acceptance remain explicit limits in the modelling guide.

The safe adapter additions are modelling.rs and references.rs; the existing
input/editing/workspace/viewport modules, ABI, native components and tests are
extended. No shared CAD algorithm, command, format or evaluator changed.
The original uncommitted Steps4–6 remain intact. Final build/gate results and
commit eligibility are in HANDOFF; nothing was pushed.

## Retained Step 6 implementation record

# Step 6 implementation status

The native sketch editor extends the Step 5 workspace, Step 4 source-aware input
and Step 3 viewport. The existing Rust engine, solver, commands, document/history
and renderer remain authoritative. See [SKETCHING.md](SKETCHING.md) for exact
bindings/gestures and [HANDOFF.md](HANDOFF.md) for measured verification and gaps.

Implemented workflows:

- Create on datum, existing construction or certified planar-face references;
  scratch plane preview/confirmation; finish and reopen the original sketch.
- Pencil lines/shared-endpoint chains, rectangles, circles, analytic three-point
  arcs and fit-point splines; live provisional previews and explicit completion.
- Plane-aware snapping/inference, independent snap/automatic-relationship
  controls, selection/multi-selection and solver-aware point/curve handles.
- Compatible geometric constraints, related-geometry inspection, selected
  indicators and removal through native history; actual solver/profile state.
- Driving dimensions through the Step 5 expression editor; preserved expressions,
  unit validation, Pencil annotation placement/movement, explicit Preview/Apply/
  Cancel and one action per confirmed edit.
- Native trim/extend, construction conversion, translate/copy/rotate/mirror;
  explicit whole-sketch deletion with native dependent-feature information.
- Bounded latest-work previews, stale tickets, actual-input commits, safe
  Pencil/finger cancellation and planar pan/zoom. Primitive previews evaluate only
  through the scratch sketch; native live drag defers downstream regeneration.

The deterministic core acceptance suite verifies native profile closure,
anchoring and dimensions, genuine conflict recovery, curve edits, history,
native container round trips and the rectangular prism's 12,000 → 18,000 mm³
regeneration. Runtime coverage is recorded separately; a build is not Pencil/UI
verification. The physical iPad is unavailable at the user's latest instruction.

| Area | Integration files |
| --- | --- |
| Safe Rust | New `session/src/sketching.rs`, `sketch_input.rs`; extended `editing.rs`, `workspace.rs`, `input.rs`, `viewport.rs`, `lib.rs`; existing `tests.rs` |
| Native controls | `WorkspaceController.swift`, `WorkspaceComponents.swift`, `WorkspaceLayout.swift`, `NativeSession.swift`; existing source registration |
| ABI | `bridge/src/ffi.rs`, existing `ffi/tests.rs`, `include/solidcraft_ipados.h`; ABI 3 C symbols unchanged |
| Evidence/text | `bridge/examples/render_proof.rs`, existing native layout/focus harness; pending `ipados-sketching.json` |
| Documentation | This file, `HANDOFF.md`, `SKETCHING.md`, architecture delta note and build commands |

Paths are relative to `crates/ipados/` or
`apps/solidcraft-ipados/SolidCraft/`. Prior uncommitted Steps 4–5 remain intact.
Step 6 changes no upstream engine/kernel/solver source, dependencies, file format
or signing/toolchain configuration.

Remaining: physical UIKit/Pencil/touch/keyboard/VoiceOver checks and screenshots;
additional native tool variants (centre arcs, control-point spline authoring,
spline insertion, path trim, transform scale, richer automatic tangency).
Control-point spline insertion is an actual engine limitation; other unexposed
variants are native UI scope gaps. General solid modelling/timeline UI is Step 7;
assemblies and the native document browser/persistence remain later work.
