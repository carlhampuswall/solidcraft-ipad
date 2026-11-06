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

## Ownership and contracts (Steps 3–4)

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
- `crates/ipados/bridge`: ABI **3**, checked integer handles (maximum 8), bounded
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
  from the same evaluated state. Visible sketch profiles retain the engine's
  world-space planes, native regions/holes and worker-prepared feedback meshes.
  B-rep faces come from `Mesh::tri_face`, edges
  from `Mesh::edges`, vertices from the engine edges' endpoints. Display-only
  mesh outlines are excluded from edge/vertex selection. World transforms and
  visibility come from `Session::world_state` and `view::scene_with_guides`.
- `RenderSnapshot` retains the scene and the display camera/projection, guides
  and highlight scenes. The GPU model upload is keyed by the retained immutable
  display scene Arc, changing only for geometry/preview publication. Body tint references its existing GPU block; independent
  guide/highlight keys change only when their actual geometry changes.
  Clipping uses shared `Camera::view_proj_bounds`; the shared renderer handles
  forward/reversed GPU depth conversion. No idle render timer is installed;
  input, resize, command completion and bounded drawable retries request frames.
- Publication has a request generation, document identity and geometry revision.
  Superseded publications and callbacks after close are discarded. Suspension
  cancels provisional edits; an already-dispatched ordinary document command may
  finish and publish its immutable scene for foreground reuse, with GPU submission
  still disabled. See INTERFACES.md and the bridge suspension regression. The previous scene
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
- Step 4 extends that same `InputEvent`/`Viewport` boundary with per-contact
  ownership, actual/predicted status, ordering and presented viewport/camera epochs.
  The separate Pencil tool lifecycle supports suspended navigation and safe rebasing.
  See [INPUT.md](INPUT.md) for the exact mapping, state and cancellation contract.
- `editing.rs` evaluates preview commands on upstream scratch sessions, builds
  scenes with the existing pipeline, and prepares `ComputedCommands` for one atomic
  `Session::accept_commands`. No provisional document is ever installed into the
  authoritative session. The viewport owns only immutable preview scenes and a
  latest-work mailbox; the engine lane consumes one work item at a time. Acceptance
  rechecks the interaction ticket while holding the viewport lock, after evaluation.
  Native input buffers are bounded; no touch object crosses a queue.
- Preview frames retain the authoritative document/geometry input stamp while the
  renderer keys uploads by the actual immutable display scene Arc. Body feedback
  and handle rendering follow the evaluated preview. Camera input remains usable
  during evaluation. Lifecycle suspension cancels tools even without an attached
  surface and rejects late publications.
- Native project browsing, coordinated security-scoped imports, autosave and
  last-project restoration wrap the existing native file commands. The project
  transaction prepares/evaluates/saves a candidate before replacing the live
  session; a save-only request preserves provisional viewport work. Exports use a
  frozen native container and a separate disposable worker session. See
  INTERFACES.md for ownership, supported formats and limits. No new CAD format
  or kernel change is introduced. [Project insertion](PROJECT-INSERTION.md) uses
  the existing shared Insert Design command's optional `link: false` snapshot
  mode, retaining its default linked behavior for desktop and web callers.

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

The [2026-11-04 upstream integration](UPSTREAM-INTEGRATION-2026-11-04.md) adopts
upstream's solidcore kernel and solidcore-record geometry data boundary. The
earlier truck-only `blend.rs` tangent plane/cylinder rail chamfer extension is
retired with that backend; solidcore rejects smooth tangent joins. Its analytic
regression is preserved with the integration evidence. The exact upstream kernel
pin has a vendored, one-line Rust 1.95 slice-coercion fix; geometry is unchanged.
Current compatibility checks and limitations are in the integration record.

`cmd/face.rs` and `cmd/features.rs` partition multi-face extrusions by oriented
plane and capture all linked profiles before applying any extrusion. Each plane
uses the existing Extrude feature along its own normal; the engine transaction
keeps the batch atomic. Coplanar selections retain their single feature. This
Apple-independent command improvement adds no document format or kernel changes.
The adapter's tool preview includes every newly created extrusion body.

Face-based operations mark their generated linked boundary with the additive
`Feature.internal_profile` flag (`document.rs`, `placement_reuse.rs`,
`cmd/sketch.rs` and `format/pending/internal-face-profiles.rs`). The native browser,
timeline and profile choices keep it internal; shared `view::sketch_shown` excludes
it from normal display, and implicit profile commands choose the last authored
sketch (`cmd/features.rs`). Evaluation, linked references and atomic history keep
their existing paths. Unmarked older sketches retain their previous meaning.
This is an Apple-independent provenance improvement suitable for upstream;
desktop/web retain their existing tree presentation and explicit ID access.

`Document::delete_features` removes an internal face profile when its last
consuming operation is deleted, in the same command/history step. Authored or
still-referenced profiles remain. This small shared lifecycle correction keeps
hidden face references from outliving their operation; no format or kernel
change is involved. The native workspace can publish the shared evaluator's
current partial state after feature errors and expose failed internal profiles
from older saves. Preview acceptance and working-recovery validation retain their
strict complete-model checks; stale scenes are never published as current.

`crates/doc/src/profile_refs.rs` also preserves a complete drawn profile by its
whole source boundary when projected face edges split it into ambiguous graph
regions. Previously the shared capture could retain list indices; resizing the
supporting sketch could silently select different regions and cut away an entire
body. The existing whole-boundary representation now resolves all pieces of the
same drawn profile, with complete area coverage checked. Partial picks retain
their exact region. `format/pending/drawn-profile-boundaries.rs` gates this saved
meaning for older readers. This is a shared document-reference fix suitable for
upstream; it adds no platform dependency or kernel behavior.

| File outside platform directories | Why adapter alone is insufficient; minimal change | Desktop/web impact; upstream suitability |
| --- | --- | --- |
| `Cargo.toml` | Register two actual nested packages; retain native host/namespace exclusions. | Adds members only; no changed shared dependencies. Fork-specific registration. |
| `Cargo.lock` | Cargo records the two local packages. | No external version changes. Fork-specific. |
| `crates/engine/src/cmd/component_link.rs` | Temporary native Files imports need an embedded snapshot in one undo step; `component.insert_design` accepts `link: false` and detaches after property inheritance. | Default `link: true` preserves existing callers. No Apple dependency or format change; suitable for upstream snapshot insertion. |
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

Native reference axes use `GpuScene::guide_lines` in the sketch/guide slot.
The shared renderer draws that slot's reference lines before model/sketch
strokes, retaining depth testing against solids; coincident sketch strokes
therefore keep their own colour. Existing sketch lines and feedback keep their
ordering. `Capturer::rgba_viewport_with_guides` exposes the same separate guide
slot for offscreen verification and clears it between captures. These additive
shared GPU seams introduce no Apple dependency; upstream integration must retain
the guide-line draw and the capture slot until equivalent upstream APIs exist.

Step 4 changes only the native host/adapters, a pending locale file and iPad
documentation. No further shared-source or dependency delta is required. Its
maintenance surface is the existing public scratch/transaction, timeline-edit,
scene, camera and selection APIs; optional UIKit APIs remain entirely in the host.

Step 5 adds thin UIKit workspace components inside that same host boundary and
`session/src/workspace.rs`: revision-cached document presentation and a numeric
transaction adapter over the same upstream APIs. Browser selection calls the
existing viewport selection path. Numeric and Pencil previews share immutable
scene publication and GPU blocks; history remains authoritative in `Session`.
Native preferences stay in UserDefaults. Aspect correction delegates to shared
camera navigation. No shared source, dependency, file format, solver, renderer or
egui rule changes. See [UI.md](UI.md).

Step 6 extends that same boundary with `sketching.rs` (actual world sketch and
inspection/glyph presentation) and `sketch_input.rs` (native tool state and
command proposals). `ToolOperation::Sketch` shares the radius tool's revisioned
mailbox, scratch evaluation and atomic acceptance. Driving dimensions generalise
the existing numeric transaction; expressions remain upstream parameters.
Plane preview resolves `sketch.create` on scratch and keeps the original attachment
value for confirmation. No shared-source, dependency or format delta is added.
Upstream update seams are the existing public sketch types/shape/dimension layout
helpers, inspection schemas, solver diagnostics and command contracts listed in
[SKETCHING.md](SKETCHING.md). No large frontend module is copied.


Step 7 adds `modelling.rs` presentation forms and `references.rs` strict native
reference checks at the same platform boundary. The one Session evaluates on
scratch, and ComputedCommands/accept_commands adopt a prepared operation after
interaction/version/document validation. The authoritative history marker never
moves merely to open a form. Feature editing displays a scratch state through
that feature, or its preceding source state while picking references; previews
exclude the downstream suffix. Apply uses the existing computed-command marker
restoration to rebuild through the latest feature in one Undo step. Cancel and
lifecycle suspension restore the original scene. Sketch editing uses the shared
`sketch.edit` temporary marker and restores the latest feature on Finish/Cancel.
The native `rollbackFeatureEditing` preference retains full-history editing.
Failed-feature repair uses the same scratch source state and original feature.
Handle work shares the existing
bounded mailbox, with exact parameter/version checks and explicit confirmation.
Native file services save/reopen the existing full container in one app-private
slot. No engine/kernel/document source, format or dependency delta is added.
See MODELLING.md for command mappings and HANDOFF.md for observed verification.

## Step 8 assembly boundary and shared deltas

`assembly.rs` presents the existing component/occurrence/joint commands through
the same ModelEdit forms. `assembly_references.rs` checks the actual native
reference/resolve APIs before preview or Apply. It owns no geometry or topology
matching. Joint/frame IDs and full occurrence paths remain native; ambiguous
repeated nested endpoints are refused because upstream JointOrigin stores only
an occurrence ID. Revision-cached browser/diagnostic data is presentation, not a
second assembly graph or solver. Legacy circular endpoints without an explicit
end retain upstream semantics; this step does not solve persistent naming.

Placement, joint creation/edit/drive/reach and Pencil handle proposals evaluate
on native scratch sessions and accept through the existing ComputedCommands
contract. Actual native results (values, moved/reasons, clamps, residuals and
conflicts) determine validity. The viewport never owns mutable component poses.
Pending placement capture/revert is explicitly scoped and does not cancel
unrelated pending branches. One completed Apply is one native history action.
Handle, numeric and slider proposals retain one display-only last-good scene
while the newest job is pending. It does not validate the current parameters: `preview_stale` labels
that display and disables confirmation until the current result arrives. This
avoids flashing back to the committed pose between input samples. Cancellation
or a changed context discards the provisional display; no pose is saved by it.

Immutable scene snapshots carry public native `mesh_in_place` source meshes and
complete effective transforms. The native renderer's bounded 256-entry source
Arc/SharedTris cache feeds existing InstancedMesh APIs; placement-only revisions
reuse triangle data. Selection/hover identifies a world occurrence instance.
World edge/sketch streams still upload, and per-face/translucent appearances use
the existing geometry fallback. The cache owns no CAD transform state, and its
Arc retention plus monotonic keys prevents pointer-reuse aliasing. No shared GPU
implementation or frontend is copied. No upload/latency benchmark is claimed.

| Shared file | Why adapter alone was insufficient; minimal change | Compatibility and maintenance |
| --- | --- | --- |
| `crates/engine/src/cmd/component.rs` | Capture Position previously captured every pending occurrence. A zero-move workaround could refuse grounded/rigid chains. Add optional `occurrences` IDs, validate all before mutation, then capture only them. | Omission preserves capture-all, empty list captures none. No format change. Existing occurrence tests cover atomic validation/history and unrelated pending preservation. Independently upstream-suitable; maintain the optional command argument during future merges. |
| `crates/engine/src/cmd/joints.rs` | Joint edit lacked an origin rebind path; the adapter would need direct mutation or deletion/recreation. Add optional A/B origin parameters through the existing parser, resolve both before mutation, retain identity and use the existing solver. | Existing callers are unchanged without A/B. No JointOrigin/format/solver change. Existing joint tests cover edit/invalid side/history/native roundtrip. Independently upstream-suitable; maintain the additive parser fields with future command changes. |

All other Step8 changes stay in the platform host/adapters, pending messages,
existing area tests, render proof and documentation. Native file services retain
the full document container, including assembly data the UI cannot edit. See
[ASSEMBLIES.md](ASSEMBLIES.md) for capability limits and [HANDOFF.md](HANDOFF.md)
for exact verification and native file/recovery integration seams.


## Step 8.5 consolidation review

All Steps 1–8 were found in the original checkout. The three existing commits
remain ancestors of `integration/ipados-foundation`; later milestones were
already layered in one uncommitted workspace, so no merge/cherry-pick or invented
per-step history was needed. [CONSOLIDATION-INVENTORY.md](CONSOLIDATION-INVENTORY.md)
records source provenance and preservation. [INTERFACES.md](INTERFACES.md) is the
reviewed map of actual types, ownership, threads, revisions, coordinates, errors
and cancellation. [PARALLEL-DEVELOPMENT.md](PARALLEL-DEVELOPMENT.md) controls future
file ownership and launch eligibility.

The audit found no duplicate authoritative document, CAD command registry,
geometry kernel, sketch solver, undo stack or camera controller. Native form
state and cached workspace/scene data are presentation/provisional state. They
never replace the authoritative session. Native feature and joint edits retain
original IDs; regeneration invalidates topology selection conservatively.
Input uses view-local display points and presented-state stamps, while evaluated
occurrences and picking use the same native world transforms.

Suspension has two distinct contracts: provisional interaction tickets are
cancelled; ordinary commands already dispatched may complete and retain their
authoritative scene while rendering is inactive. The bridge regression
`suspension_retains_dispatched_command_results_but_rejects_cancelled_preview_commits`
checks this using a held engine lock, real command dispatch, scene publication,
stale-preview rejection and undo/redo. No extra cancellation/command adapter or
scene-refresh workaround was introduced.

### Compiler compatibility delta

The pinned Rust 1.95 full gate exposed inherited lints successively through the
workspace. Consolidation makes only equivalent condition/match rewrites in the
following shared files; no lint is suppressed, algorithm replaced, dependency
upgraded, format changed or default changed:

| Shared files | Maintenance implication |
| --- | --- |
| `crates/geom/src/fill_general.rs` | Equivalent positive finite-tolerance guard. |
| `crates/kernel/src/coplanar.rs`, `curveblend.rs`, `ops.rs`, `step_in/brep.rs`, `tess/grid.rs`, `tess/seam.rs` | Boolean algebra only; original short-circuit/NaN guards and sampling conditions retained. No kernel feature work. |
| `crates/doc/src/expr.rs`, `crates/io/src/design.rs` | Equivalent absent-value/object-shape guards; native format unchanged. |
| `crates/engine/src/cmd/component_clone.rs`, `sketch_modify.rs` | Equivalent match guards and finite arc validation; external-reference notices unchanged. |
| `crates/ui-egui/src/agent_chat/paste.rs`, `browser.rs`, `canvas.rs`, `dialogs.rs`, `dialogs_dense.rs`, `sketch_tools.rs` | Equivalent UI predicates; explicit match fallbacks preserve existing positive/NaN span and hidden-row behavior. |

These are independently upstream-suitable compiler cleanups. Reconcile/drop them
when upstream supplies the same fixes; do not retain competing implementations.
The optimized macOS crash-report test also needs its `child` helper marked
`#[inline(never)]` in `apps/solidcraft/src/native_crash_macos.rs`: its assertion
requires that caller frame to exist. All crash assertions and production handler
code remain unchanged. The 12 focused crash tests pass with this test-only fix;
reconcile it with upstream changes to the frame-walk fixture.
The Step 8 command extensions and earlier navigation/capture seams remain the
only behavioral shared API changes described above. No new external dependency
or new shared API is added by Step 8.5.

### Native catalog isolation

The complete desktop translation gate exposed 372 UIKit-only `ui.ipados.*`
messages entering the desktop/web catalog. `crates/ui-egui/build.rs` now excludes
that namespace after validating the source catalogs and duplicate IDs. The
existing completeness assertion remains unchanged; tests in `src/i18n.rs` check
native exclusion and preservation of every shared source message. This also keeps
native captions out of desktop legacy label lookup.

`xtask/src/i18n.rs` uses the same domain for coverage reporting, while its raw
catalog reader/writer preserves all native entries. Its fixture checks both
pending and folded catalogs and still detects missing shared translations. The
native locale generator continues reading the original source/pending catalogs
with offline English fallback. No generated catalog or translation was replaced.
These three small shared-file changes are fork-specific catalog integration;
reconcile the namespace boundary when upstream adds its own platform catalog
domains, keeping native extraction and shared completeness intact.

### Remaining constraints

NativeSession and the workspace controller are concentrated modules. Parallel
work must respect the ownership matrix rather than giving every task unrestricted
access. Full Files/security-scoped access/recovery is absent and belongs to Task A;
measured rendering performance to Task B; physical accessibility acceptance to
Task C. Shared protocol and lifecycle wiring remains the integration owner's work.
Native repeated-nested joint-origin ambiguity and unexposed tool variants retain
the limits documented in ASSEMBLIES.md and MODELLING.md. Historical physical
Step 8 evidence does not certify all sketch/modelling or simultaneous hand-input
workflows. Current verification is recorded only in HANDOFF.md.


### Development baseline exception and preserved checkpoint

On 2026-10-28 the user resumed consolidation and accepted the three already
reported upstream macOS engine failures for the development baseline only. The
exact ring-volume and seed 36/71 observations, evidence and follow-up conditions
are in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md). Tests remain
enabled and expectations/tolerances unchanged; full CI must still report its
actual failure. A new failure or changed signature blocks approval. Seed 36's
invalid geometry remains a real limitation, not an accepted correctness result.

[CHECKPOINT.md](CHECKPOINT.md) preserves the earlier blocked decision and
interrupted duplicate run as historical records, explicitly superseded by this
limited user exception. Source baseline B is now `c22acd12702cf3a405df01009b8d8662b3d46365`. The completed full `cargo +1.95.0 xtask ci` run exits **1**: **85/86 top-level checks pass**; the engine library has **2631 passed, 3 failed, 6 ignored**. Only the three unchanged failures in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) remain under the user exception. All other required checks passed. **CI is not green.**

Tests remain enabled; expectations, tolerances and goldens are unchanged. Any new failed test or changed failure signature blocks acceptance. Recheck the exception after a relevant upstream fix, affected engine/kernel/toolchain changes, and before release. No automatic monitor was created.
The finalized policy is read from the documentation-only descendant P recorded in the final report; every task starts at B and still requires an explicit launch request.

### Preserved user-requested history gestures

After the checkpoint, the separate “Add multitouch undo and redo” task added
two-finger double-tap Undo and three-finger double-tap Redo in `MetalCanvas.swift`
and `WorkspaceController.swift`, with `INPUT.md` and pending
`ipados-history-gestures.json`. This authorized work is included in the candidate.
The native recognizers observe direct-touch taps without synthesizing CAD
contacts or delaying navigation; callbacks use the existing engine history and
controller eligibility rules. They add no history stack or camera controller.

The user-requested default is enabled. Tools exposes
`SolidCraft.workspace.historyGestures` to restore the earlier behavior; UIKit
text fields retain their editing gestures. The separate task's unsigned build
and 186 existing layout cases passed, while physical gesture testing remains
unperformed. The combined candidate's fresh unsigned native build and all 186
Foundation cases pass; the completed full-CI result and committed B are recorded above. The added
label brings the native English catalog to 373 keys; the earlier 372-key
isolation proof remains historical. The separate UI overhaul task made no source
changes and is waiting for the baseline.
