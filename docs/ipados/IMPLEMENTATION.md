# Step 8.5 consolidation status

**Resumed; development baseline pending fresh validation and commit.** On
2026-10-28 the user accepted only the three already reported upstream engine
failures for continued development and asked that they be documented for later
checking. [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) records exact
observations, upstream links and follow-up triggers. Tests remain enabled and
expectations unchanged; CI is not relabeled green and any new failure blocks
approval. The blocked decision in [CHECKPOINT.md](CHECKPOINT.md) is historical
and superseded by this limited exception. Current results are in HANDOFF.md.

Steps 1–8 are real implementation in the original checkout. The dedicated
`integration/ipados-foundation` branch preserves the Step 1–3 commits and the
accumulated Steps 4–8 working files. No milestone was recreated, source history
squashed or upstream update imported. No next-milestone feature was implemented
by this consolidation task. The consolidated
candidate is uncommitted; HEAD remains the Step 3 ancestor, not a launch baseline.

The audit confirms one authoritative session/dispatch/history, central input
ownership, shared navigation, native transforms and scratch preview/atomic
acceptance. [INTERFACES.md](INTERFACES.md) records actual contracts. Consolidation
adds a deterministic suspension regression, seventeen equivalent compiler lint
repairs, a test-only optimized crash-fixture correction and native catalog-domain
isolation. ARCHITECTURE records their shared maintenance surface.

The earlier candidate checks passed 105 session / 24 bridge tests, 186 native Foundation cases,
87 desktop-app and 2,932 UI tests, full-workspace Clippy and repository gates,
wasm/doc jobs, dependency isolation, web offline/update, pending-format checks
and the unsigned Rust/UIKit build. Real Mac GPU and independent CLI workflows
verify sketch/modelling/assembly transactions, geometry and native persistence.
Physical iPad and simulator testing were not performed in this consolidation.

That complete gate failed on three pre-existing macOS engine geometry
regressions. All three reproduce in the preserved older binary and match
upstream reports; their expectations were not changed. The first full run
completed with those failures. The repaired rerun passed 85/86 checks and again
reported the same three failed engine tests before its duplicate engine process
was stopped for the earlier checkpoint. It is explicitly not a passing full gate.
Those exact failures alone no longer block the user-approved development
baseline; a fresh full run and all other mandatory checks are still required.

Additional user work from “Add multitouch undo and redo” is preserved in the
combined candidate: two-finger double-tap Undo and three-finger double-tap Redo
in `MetalCanvas.swift`/`WorkspaceController.swift`, updated `INPUT.md` and pending
`ipados-history-gestures.json`. It uses existing engine history and defaults on;
`SolidCraft.workspace.historyGestures` restores the old behavior. The separate
task's unsigned build and existing 186 layout checks passed; physical gesture
testing remains outstanding. The UI overhaul task remained read-only and added
no implementation. Fresh combined unsigned native compilation and all 186
Foundation cases pass in `/tmp/solidcraft-step85-resume-native-{build,check}.log`.
The history label increases the native English catalog from the historical 372
to 373 keys. Fresh full CI and source baseline B remain pending. No consolidation
commit, push or parallel-task launch has occurred.
Both parallel documents still require the exact source baseline SHA before use.

The sections below are retained milestone records. Their earlier “next step,”
branch names, test totals and device availability statements are historical;
current scope/ownership and validation are above and in HANDOFF.md.

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
