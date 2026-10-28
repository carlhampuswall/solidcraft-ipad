# Parametric modelling and feature timeline (Step 7)

Step 7 builds on the existing native workspace, sketch editor, input router and
immutable viewport scenes. Modelling remains in the shared engine and document
evaluator. The platform additions are forms, transaction tickets, input feedback
and reference validation; there is no native geometry engine, copied feature
definition, second document evaluator or separate modelling undo stack.

## Capability map and command bindings

The checked-out revision exposes all the requested basic operation families.
`crates/engine/src/cmd/features.rs` is the command authority. Extrusion also has a
typed contract in `crates/engine/src/schema/features.rs`; several other operations
are still listed in `schema/unconverted.txt`. Native forms explicitly map the
parameters the handlers actually read rather than assuming schema completeness.

| Checkpoint / tool | Implemented native binding | Current presentation scope |
| --- | --- | --- |
| 7A Extrude | `solid.extrude`: sketch/profile, distance, direction, through_all, operation, targets | Native classified region, including its holes; positive, negative or symmetric total distance and Through All; new body, join, cut and intersect. Explicit target list for Boolean modes. |
| 7A Fillet | `solid.fillet`: edges, radius, body, type, `fit:exact` | Multiple actual B-rep edges; constant-radius, chord and supported curvature modes; real kernel preview and later input edit. No automatic radius reduction. |
| 7A Chamfer | `solid.chamfer`: edges, distance, distance2/angle, body | Multiple actual B-rep edges with equal, two-distance or angle creation; stored native numeric inputs remain editable. |
| 7A Feature editing | `timeline.inspect`, `timeline.edit`, `parameters.change` | Original feature ID and order retained; full definition preserved; expressions loaded through the native input parameter rows. |
| 7A Timeline | Real `Document.features`, `Model.result`; `timeline.rename`, `timeline.suppress`, `timeline.dependents`, `timeline.delete` | Ordered names, types, suppression, errors, warnings and stale state; dependency-aware delete confirmation. |
| 7B Revolve | `solid.revolve`: sketch/profile, axis, angle, operation, targets | Sketch/world axes and the native axis choices provided by the form; basic angular extent and explicit Boolean targets. |
| 7B Shell | `solid.shell`: faces, thickness, direction, body | Selected removal faces, inside/outside/both wall direction, original thickness editing. |
| 7B Hole | `solid.hole`: sketch, diameter, depth, targets, `type:simple` | Real sketch points/circle centres, blind depth or Through All and explicit drilled bodies. No fabricated specialised hole variants. |
| 7B Boolean | `solid.combine`: target, tools, operation, keep_tools | Join/union, cut/difference and intersect; target and tool roles explicit; retain/consume choice visible. |
| 7B Patterns | `solid.pattern.rectangular`, `solid.pattern.circular`, `solid.pattern.path` | Explicit body-copy or feature-replay source mode; rectangular count/spacing/direction, circular count/angle/axis, path count/spacing/ordered curves/orientation. |
| 7B Mirror | `solid.mirror`: bodies or features, plane | Explicit body-copy or feature-replay source mode about supported plane choices. |
| 7C Sweep | `solid.sweep`: sketch/profile, path_sketch, ordered path curves, twist, scale, operation, targets | Basic native profile/path sweep, ordered curve list, twist/end-scale expressions and explicit Boolean targets. |
| 7C Loft | `solid.loft`: ordered sections, operation, targets | Ordered native classified sections; list order can be changed before preview. |
| Named parameters | `parameters.add`, `parameters.change` | Native names, units and expression validation; no independent expression parser. |
| History and persistence | `edit.undo`, `edit.redo`, existing `file.save`/`doc.open` services | Native document and history semantics; app-local save/reopen is available without adding the Step 9 browser. |

The engine supports more options than these initial native forms expose. They
include extrusion Two Sides, start offsets, draft and To/From Object;
variable/spline fillet styles, additional hole variants,
arbitrary axis/plane construction, guide rails and
advanced loft/sweep options. Existing features containing these options are
preserved when saved; a native form must not silently simplify them.

Pattern/mirror source mode is a transient form choice. Body copies and replayed
features have separate visible input lists; only the selected source list reaches
the constructor. Editing uses native feature IDs and explicitly clears the
inactive source list, because `timeline.edit` merges definitions. Feature choices
are restricted to supported replay kinds earlier in the feature's history;
downstream or suppressed sources cannot be introduced. Source renames and upstream
parameter edits continue through the shared native relationships.

Path patterns reuse the ordered sketch-curve selection form. Creation maps to
native `path_sketch`, `path`, `count`, `spacing` and `orientation`; editing retains
the original `PatternKind::Path` and updates its native nested fields, without
recreating the feature. Through All holes omit `depth` on creation or set the
native optional depth to `null` on edit. Extrusion stores the existing native
extent flag. No new persisted platform field is introduced.

Known upstream restrictions include unsupported G2 fillet corners where the
kernel reports that limitation, curved sections lofted to an apex, incompatible
section hole counts and coplanar loft sections. These do not require a new kernel
and do not prevent the supported basic workflows. Arbitrary fillets, shells and
Booleans are not promised to succeed.

## Selection and targets

Profile choices come from solved sketch regions and upstream
`capture_profile_selection`; neither rendered triangles nor screen-space loops
classify material. The selected region remains associated with its source sketch
and document/geometry revision. Region labels include the sketch and region area.
Internal holes use the native profile representation.

Pencil selection uses existing native `Sel` body, face and edge references. An
edge index is only a current-revision input to the upstream constructor, which
stores its native persistent names/reference information; no GPU or tessellation
triangle identity is saved as feature history. Face removal and multi-input
lists remain visible and individual entries can be removed. The selection action
removes a provisional solid before acquiring more inputs, so the displayed
source and its picker agree. Tool-first forms
can refresh eligible selections through the same bridge interaction.

Join, cut, intersect and drilling require explicit native target bodies. The
adapter refuses an empty target set instead of using the engine's broader
automatic scope. Combine separately identifies its target and tools, and exposes
the engine's retain/consume behaviour. No automatic unrelated body consumption
is introduced.

The extrusion handle follows the evaluated sketch plane and normal, including
the existing body/component transforms. Handle movement changes the distance
proposal; it does not move a display mesh independently. Finger navigation stays
on the shared camera path. Native controls consume their touches before canvas
input and text focus uses the established workspace focus policy.
The distance handle is hidden for Through All and other extents where changing
distance would not control the displayed endpoint. Legacy symmetric extents keep
their original per-side convention.

## Preview, confirmation and cancellation

The engine lane owns the authoritative `Session`. Native `ModelEdit` holds only
provisional form data, a token, version, document identity, geometry revision and
an immutable preview scene. `prepare_model` evaluates existing commands on
`Session::scratch`; `ComputedCommands` and `Session::accept_commands` adopt the
prepared result after source-context validation. Expensive evaluation and scene
construction happen without the viewport or renderer mutex held.

The C ABI remains version 3. Engine-lane JSON `kind:model` adds begin, inspect, selection,
preview, commit and cancel actions. Begin can accept initial form overrides;
inspect refreshes the choices and draft. Parameter/selection changes invalidate
the prior preview and advance the interaction version. Each evaluated result is
checked against the current token/version and document context before publishing
or accepting. Older results are discarded. A consumed token cannot commit twice.

The host disables Apply while parameters change or computation is pending, and
enables it only after a current successful engine preview. Invalid input removes
current-preview status and returns structured diagnostics. Confirmation evaluates
the submitted current parameters again before acceptance. A successful operation
or edit is one logical engine undo action; typing, handle samples, navigation and
cancel do not append history entries.

Pencil work reuses the existing bounded latest-work mailbox and lifecycle events.
Ending a modelling handle creates a proposal; explicit Apply commits it. There is
no unsafe kernel preemption: superseded work may finish, then fail its ticket
check. Cancellation drops provisional state only. Lifecycle suspension also
invalidates tokens without waiting for the engine worker or requiring a GPU
surface. Ordinary document mutations and concurrent numeric transactions are
blocked while a model draft exists; viewport navigation and resizing remain
available.

## Feature editing, dependencies and failures

The timeline comes from the document's actual feature order, independently of
Undo/Redo. Selection exposes the original feature definition, expressions and
native IDs. Editing uses `timeline.edit` on that ID, preserving its place in the
chain. Selection choices for edits are built from scratch history before that
feature, so downstream-only bodies/sections are not offered. The authoritative
history marker is never moved merely to open a native feature form.

Reference validation in `crates/ipados/session/src/references.rs` checks each
active feature against `Model::state_before(feature)`. It calls the upstream
strict profile resolver and native named edge/face lookup. Missing, split or
fallback references produce an error naming the affected feature and requiring
explicit reselection. The adapter does not implement general persistent topology
naming. Native fallback-reference warnings and stale/failed evaluation are not
labelled as a successful current model.

Public `Session::execute` restores its snapshot when a command returns an error.
A successful upstream edit can still report failures on later features; the
native layer surfaces those diagnostics and retains recoverable document state.
A failed publication retains a clearly labelled previous scene. Opening an
existing failed feature instead presents its actual upstream scratch state;
explicit profile/edge/face reselection repairs that same ID. Cancelling retains
the failed document and disables current-model picking. Neither display is
labelled as a successfully regenerated full model. Undo remains available.

Rename and suppression use native commands. Before deletion, the UI reads
`timeline.dependents` and explains both `deleted_with_it` and `would_fail`.
Deleting a sketch may delete its native dependent features, only after that
explicit confirmation. Feature reordering and a rollback-marker UI are not
implemented by moving visual rows. Native commands exist, but these remain
unexposed rather than implying a tested touch workflow.

## Parameters and native persistence

Expressions, unit strings and named parameters pass through the shared engine.
Opening an unchanged field retains its stored expression rather than substituting
an evaluated number. Invalid units, unknown parameters and cycles use upstream
diagnostics. Unedited native definition fields and unknown/unexposed feature
types remain in the document. Previews, selection lists and interaction tickets
are never added to the CAD format.

The app-local Save/Reopen action uses the existing native container at
`Application Support/SolidCraft/Modelling.solidcraft`. It is a focused persistence workflow; Files-app
coordination, security-scoped browsing, autosave and restoration remain Step 9.
The shared native writer/reader retain sketches, constraints, feature order,
expressions, target/reference relationships, suppression and document metadata.
Saving final tessellation alone is not used as document persistence.

## Verification and integration

`cargo +1.95.0 test --profile ci -p solidcraft-ipados-bridge --lib -- --test-threads=1`
passed all **19 bridge tests** on 2026-10-28, including four modelling cases:

- Real 40 × 30 sketch extrusion preview at 10 mm (12,000 mm³), isolated preview,
  invalid text, single commit, duplicate-confirm refusal, Undo/Redo and editing
  the original feature then cancellation.
- Lifecycle suspension after preparing an actual commit, with no attached GPU
  surface, invalidates the result and preserves the document.
- Deterministic newer-before-older preview completion rejects the older result;
  a document change before acceptance rejects the prepared transaction.
- Inspecting a draft preserves current validation when unchanged and invalidates
  it when parameters change; tokenless dismissal cancellation is idempotent.

These are real engine and bridge/controller tests, not physical Pencil or UIKit
runtime results. Complete acceptance fixtures, final build/gate commands and
device coverage are recorded in [HANDOFF.md](HANDOFF.md). No physical runtime,
screenshots or performance result should be inferred from a successful build.

Step 7's CAD-facing additions are confined to `crates/ipados/session` and
`crates/ipados/bridge`; native presentation remains in `apps/solidcraft-ipados`.
No new kernel algorithm, command implementation, native document format or shared
upstream evaluator patch is introduced for this milestone. Future upstream
integration should recheck handler parameter names, feature-definition shapes,
profile capture/strict resolution, named-reference lookup and `ComputedCommands`
source-context semantics. The existing shared desktop/web behavior is retained.

Step 8 can use this authoritative timeline, body/reference choices, shared
parameter inspector and transactional preview boundary for assembly presentation.
Assembly/joint editing and the native document browser are outside Step 7.


Final measured acceptance:82 session tests (23 added for Step7),19 bridge tests,
176 native adaptive cases plus production response/JSON checks, affected Clippy,
all14 wasm crates, web offline harness and unsigned iPhoneOS compile/link passed.
Mac GPU captures include preview, original depth editing, Undo/Redo and a
parametric circle cut. The separate upstream CLI reopened the saved four-feature
container, preserving dimensions and the named radius expression, with one closed
body and volume17,575.9554388 mm³ (analytic18,000−135π within the approved curved
mass tolerance). This is not desktop UI or iPad/Pencil runtime verification.
The full repository commit gate remains blocked by the inherited shared
nonminimal_bool lint in geom/src/fill_general.rs166; see HANDOFF for exact logs.
No commit or push was made. Physical UIKit/Pencil/VoiceOver verification and
latency measurements remain outstanding.
