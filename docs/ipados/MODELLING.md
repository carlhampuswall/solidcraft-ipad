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
| 7A Extrude | `solid.extrude`: sketch/profile or faces/face_body, distance, direction, through_all, operation, targets | Native classified region, including its holes, or selected planar body faces; positive, negative or symmetric total distance and Through All; new body, join, cut and intersect. Explicit target list for Boolean modes. |
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
| History and persistence | `edit.undo`, `edit.redo`, existing native file commands | Native document/history semantics; the Projects browser supplies save, autosave/restoration and Files import/export. See UI.md and INTERFACES.md. |

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

Extrude also accepts a selected planar face, or up to 256 coplanar, equally
oriented faces of one body. Selecting a face while Select is idle automatically
opens the existing Extrude form with that selection as its source. **Tools →
Extrude on face selection** defaults to on and persists in UserDefaults
(`SolidCraft.workspace.extrudeOnFaceSelection`); turn it off to keep selection-only
behaviour. Active tools, sketch-plane picking and ongoing edits retain input.
Cancelling the form leaves it closed until a new face is selected; ordinary
refreshes, document replacement and geometry updates do not reopen it.

The native workspace harness checks those selection transitions and the legacy
preference. For a disposable physical-device smoke check, launch DEBUG with
`--viewport-check --viewport-check-hold-face --auto-face-extrude-check`. The
existing synthetic Pencil driver stops after selecting a face; the production
controller must log `Automatic face extrude check PASS` and display Extrude.
This checks the native input path, without claiming physical Pencil testing.
On 2026-10-29 this check passed on the connected physical iPad: the synthetic tap
selected Base face 5 and opened the production form with Base's face as its
source. Unsigned/signed builds, the native workspace harness and nine existing
face-extrusion tests passed. The unmodified device framebuffer capture retains
the existing OS orientation/clipping issue documented in HANDOFF.md.

Faces can also be selected before opening the tool or through **Select in
viewport**. The source chooser includes the selected faces alongside sketch
regions. Switching sources retains distance, direction, operation and targets.
Choose either sketch regions or body faces for one native extrusion. Curved,
noncoplanar and mixed-body face sets report errors before acceptance.
The adapter sends the upstream `faces` and `face_body` parameters; the engine
creates the linked face-boundary sketch and owns the parametric relationships.
Face-source distance handles resolve against the unchanged source body so a
Join preview cannot shift their starting plane. Apply, Cancel and undo use the
same transaction path as sketch extrusions. The operation menu offers **New
Body**, **Union**, **Intersect** and **Subtract** when creating or editing an
extrusion. All four are visible inline in the inspector and on-model distance
editor, and tapping one refreshes the actual geometry preview. New Body remains
the initial operation. A face source preselects its
own body as the Boolean target, and that automatic target follows reselection
onto another body. Explicit alternative target selections remain editable.
Union adds the extruded material; Intersect keeps its overlap with the target;
Subtract removes that overlap. Selecting Intersect or Subtract on a face points
the extrusion inward; selecting New Body or Union points it outward. This also
applies when editing an extrusion attached to a body face. A subsequent explicit
direction change is retained, and a symmetric extent stays symmetric. Signed
distance expressions retain their meaning when the direction is chosen.

Extrusion previews show the operation's material meaning: amber is a new body,
green is material added by Union, blue is the kept Intersect result against faded
original material, and translucent red is Subtract's removed material over the
actual remaining body. Operation feedback in the editor explains the colours.
These display scenes use the engine's world-space New Body tool and Boolean
result; they do not change picking, volume, undo or saved appearances. Accepted
models have no preview scene. Navigation reuses the immutable preview upload.

The opt-in DEBUG `--face-extrude-check --assembly-check-pause` driver uses the
existing native callback/frame barriers on a disposable box. It taps the real
inline Subtract button, checks the resulting inward preview, opens the distance
editor to verify its inline choices, and applies through its normal callback.
Stages are `face-inline-new`, `face-inline-subtract` and
`face-inline-distance-editor`; capture with `devicectl device capture screenshot`
and send SIGUSR1 to continue using the device workflow in BUILDING.md.

Face-extrusion regression coverage lives in the existing session tests under
`modelling_face_extrusion`. It checks selection before and after opening the
tool, source switching, measured material changes, handles, body scope,
invalid inputs, cancellation, history and native save/reopen. To capture a
40 × 30 × 10 mm block before, during and after a 5 mm face extrusion on the Mac
GPU, plus the New Body, Intersect and Subtract results (without launching an
iPad app or simulator), run:

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-face-extrude.png --face-extrude
```

Pencil selection uses existing native `Sel` body, face and edge references. An
edge index is only a current-revision input to the upstream constructor, which
stores its native persistent names/reference information; no GPU or tessellation
triangle identity is saved as feature history. Face removal and multi-input
lists remain visible and individual entries can be removed. The selection action
removes a provisional solid before acquiring more inputs, so the displayed
source and its picker agree. Tool-first forms
can refresh eligible selections through the same bridge interaction.

Each extrusion source and target field has its own **Select in viewport** action.
Target picking temporarily resolves whole bodies regardless of the saved selection
filter, accepts Pencil, stationary finger and primary pointer taps, and preserves
the source profile and entered expressions. Multiple targets can be picked or
toggled individually. Preview publication ends the temporary selection mode.

Dragging a new extrusion into existing visible material automatically chooses
Subtract and scopes it to the intersected bodies. Pulling back out restores New
Body. Detection runs on the engine worker using native world-space profile regions
and meshes, with the actual result evaluated by `solid.extrude`. Explicit operation
choices override automatic mode. **Tools > Automatically subtract inward
extrusions**, stored as `SolidCraft.workspace.automaticSubtract`, restores the
earlier manual-only behavior when disabled; the viewport setting is
`view.automatic_subtract {"enabled":false}`. This user-requested default is enabled.
Choosing Subtract on a sketch profile also points toward selected stock when only
one direction enters it. Explicit direction changes and symmetric extents remain
available. Automatic scope and selection state are provisional and are never
saved in a design.

Verification for this correction: 165 session and 28 bridge tests, all-targets
Clippy with warnings denied, adapter wasm compilation, 550 native presentation
cases and unsigned device compilation passed. The host GPU proof
`render_proof /tmp/solidcraft-subtract.png --subtract-selection` captures a real
viewport target pick, automatic inward cut preview and accepted pocket. It measures
36,000 → 34,600 mm³ and keeps the separate adjacent body unchanged. These are Mac
GPU captures; physical iPad touch behavior still needs device verification.

Manual join, cut, intersect and drilling require explicit native target bodies. The
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

## Direct handles and exact entry

Straight modelling handles use a double-ended arrow at the actual evaluated
endpoint, aligned with the projected tool axis and sized in display points.
Pencil, a finger or the primary pointer can grab the 48-point target; a finger
starting elsewhere continues through the shared navigation path. Angular assembly
handles retain their existing ring. Looking straight along a linear axis still
allows exact value entry when the drag plane cannot be resolved.

A horizontal value chip follows the presented frame, staying inside the safe
viewport and clear of the grip, panels and workspace controls. Tap it to enter
an expression in a native popover. The numeric keypad inserts digits, signs and
arithmetic; the accessory provides units, parentheses and a switch to the full
keyboard for named parameters. The original expression stays in the field.
Preview displays the engine-resolved value and provisional geometry. Apply
validates and accepts the same modelling transaction as the inspector; Cancel
abandons the operation. Dismissing the popover keeps the last submitted preview
and discards text that was never submitted. Editing alone adds no history.

The native Modify menu now includes the existing `solid.offset_face` command for
push/pull. A single planar source face supplies its outward normal and moving
endpoint; signed distances grow or shrink the native body. Multiple faces can
use exact entry in the inspector without implying one shared drag axis. Native
face lookup validates all points on one source body before preview or acceptance.
Opening or changing a preview never adopts its moved face as the next source.
Existing native face-offset features remain editable in place, including ones
with an inferred body scope. No geometry algorithm or file format is added.

Changes a default: yes — straight handles use arrows, direct finger/pointer
capture and a value chip. Turn off **On-model editing handles** in the workspace
menu to restore the cross, Pencil-only capture and inspector entry. The setting
is native presentation state under `SolidCraft.workspace.directHandles`; it is
not saved in a design. The native viewport command `view.tool_handles` accepts
`{"enabled":true|false}` and returns its ordinary diagnostic snapshot.

Verified on 2026-10-28: 125 session and 28 bridge tests; 330 native adaptive
layout cases plus value-chip placement/response decoding and icon checks;
affected all-targets Clippy with warnings denied; wasm compilation for both
adapters; catalog, layering and UI-container gates; and an unsigned device build.
The push/pull fixture checks 12,000 → 18,000 → 8,400 mm³, signed expression input,
inferred/explicit source bodies, native save/reopen, cancellation and Undo/Redo.
Finger and pointer samples verify that handle capture leaves the camera and
authoritative history unchanged, and disabling the preference restores finger
navigation. Existing tests retain Pencil, stale-work and lifecycle coverage.

`render_proof --modelling` captures actual shared-GPU extrusion and push/pull
handles and writes `-handle.json` with the presented chip value/position. Captures
under `/tmp/solidcraft-handles-*` are macOS GPU evidence, not UIKit screenshots.
Physical iPad dragging, keyboard/popover placement and VoiceOver still require
hardware verification. No simulator was launched, and no commit or push was made.
The full upstream `cargo xtask ci` gate was not run for this local UI change.

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

The host disables Apply while references refresh or computation is pending.
Apply evaluates the current parameters directly. Parameter and reference changes
preview automatically, without a manual Preview button. Invalid input removes
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
