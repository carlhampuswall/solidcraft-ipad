# iPadOS integration interfaces

This is the contract map for the implementation present at Step 8.5. It names
existing types and entry points; it does not introduce a second document model,
command adapter, input router or document service. Read [ARCHITECTURE.md](ARCHITECTURE.md)
and [UPSTREAM.md](UPSTREAM.md) with this document. Branch ownership and the approved
starting commit are governed by [PARALLEL-DEVELOPMENT.md](PARALLEL-DEVELOPMENT.md).

## Ownership and dependencies

| Boundary | Existing owner and types | Permitted consumers |
| --- | --- | --- |
| Authoritative CAD session | `crates/ipados/session/src/lib.rs`: `SessionAdapter`, privately owning one `solidcraft_engine::Session` | The bridge; adapter modules implementing native presentation/transactions; adapter tests |
| Command boundary | `SessionAdapter::execute`, existing `Session::execute`, and `bridge/src/ffi.rs` request dispatch | Native host through ABI requests; no direct Swift document mutation |
| Native transport | `apps/solidcraft-ipados/SolidCraft/NativeSession.swift`: `NativeSession`, private `BridgeState`, `BridgeError` | Native workspace and future platform services through the same `NativeSession` instance |
| Document presentation | `session/src/workspace.rs`: `SessionAdapter::workspace`, private `WorkspaceCache`; Swift `WorkspaceState` and related DTOs | Browser, inspectors, tool availability and document status |
| Scene publication | `SceneSnapshot`, `NativeMeshInstance`; `viewport.rs`: `RenderSnapshot`, `Viewport` | Renderer, picker and input; immutable snapshots only |
| Selection | `Viewport::selection`, existing engine `Sel`; Swift `WorkspaceEntity` | Browser, picking, inspectors and command selection synchronization |
| Input ownership | `session/src/input.rs`: `InputEvent`, `InputSource`, `InputPhase`, private `Contacts`; `MetalCanvas.swift`: `InputSample` | Native event capture and existing `Viewport::input`; no independent gesture controller |
| Tool transactions | `editing.rs`: `ToolEvent`, `ToolPhase`, `ToolWork`, `ToolOperation`, `PreparedTool`; `workspace.rs`: `NumericEdit`, `PreparedNumeric`; `modelling.rs`: `ModelEdit`, `PreparedModel` | Bridge dispatch, viewport interaction state and native forms |
| Feature/assembly references | `references.rs`, `assembly_references.rs`, native feature IDs, occurrence paths and joint origins | Modelling/assembly form adapters; no name-based replacement identity |
| GPU integration | `bridge/src/renderer.rs`: `NativeRenderer`, `DrawOutcome`, internal mesh cache | Surface ABI; shared GPU drawing and camera APIs remain upstream |
| Native file access today | `NativeSession` project services and `ProjectLibrary`, existing file commands | Private project library, coordinated Files import and native Files export |

The safe adapter depends on shared Rust crates and remains wasm-compatible. The
bridge adds the native ABI and GPU integration. Shared engine, document, geometry
and sketch crates must not depend on the iPad adapter or Apple frameworks. Swift
contains platform lifecycle and presentation, never CAD algorithms or history.

## Command dispatch and native transport

`NativeSession.command(_:params:completion:)` is the existing generic host seam.
It serializes a JSON `kind: command` request and chooses the viewport lane for
`view.*`, `select.*`, `tool.*` and `input.cancel`; other commands use the engine
lane. Its completion is `Result<Void, BridgeError>`: it intentionally does not
expose the underlying command payload. A future service needing result data must
coordinate that public contract with the integration owner instead of opening
another handle or calling C directly from a second queue.

`view.background` controls native viewport presentation colours. Input is
`{"color": [r, g, b]}` with three integer sRGB channels from 0 to 255, or
`{"color": null}` to restore the scene's original light background. The diagnostic
reply includes the same `background` array or null. UIKit resolves its semantic
surface colour when the canvas is dark and reapplies it on startup and appearance
changes, including System appearance. The existing Light choice restores the old
background. Optional `grid: [[r,g,b], [r,g,b]]` sets minor and major grid colours;
null or omission restores the shared palette. UIKit composites its semantic
separator over the dark surface, with subtler minor lines. All colours are
validated before changes apply. The overrides survive scene publication and
preview changes without rebuilding model geometry, changing the camera or
adding document history.

`view.grid` accepts `{"visible": boolean}` and returns the viewport diagnostic,
including `grid.visible` and `grid.colors`. It defaults to visible and changes
only the grid guide geometry; origin axes, planes, the origin point and the
orientation triad retain their separate controls. UIKit saves the toggle under
`SolidCraft.workspace.gridVisible` and restores it at session startup. Grid,
origin point, axes and planes share **Views > Visibility**. Origin controls have
moved out of Model by default; **Origin controls in Model** restores the previous
browser rows through `SolidCraft.workspace.originInBrowser`.

`view.origin` also accepts `{"group":"axes","visible":true}` or
`{"group":"planes","visible":false}` to update X/Y/Z or XY/XZ/YZ atomically.
Group operations leave the origin point and the other group unchanged. A null
`visible` restores automatic visibility for that group. `group` and `item` are
mutually exclusive; invalid requests apply no changes. UIKit's **All axes** and
**All planes** controls save the resulting per-item overrides under the existing
`SolidCraft.workspace.originVisibility` preference.

Reference planes default to half their previous width and height, with matching
rendered outlines and picking bounds. They use one neutral palette color and
leave a gap of 10% of their width beside each in-plane axis. Only the display
patch moves; the engine plane and sketch origin retain their exact positions.
XY/XZ/YZ viewport labels are hidden; axis, origin point, construction-plane and
browser labels retain their identities.
`view.origin_style` accepts `{"legacy": boolean}` and returns the viewport
diagnostic with `origin.legacy_planes`. **View > Individual references > Original
plane display** restores full-size, axis-colored reference planes touching the
axes, along with the XY/XZ/YZ labels.
UIKit persists this under `SolidCraft.workspace.legacyOriginPlanes` and reapplies
it on startup. The toggle changes guides only, without document/history changes.

`view.touch_twist` is a native viewport preference with input
`{"enabled": bool}` and the normal viewport diagnostic reply, including
`touch_twist.enabled`. It defaults to true; false restores pan/pinch-only input.
It cancels active contacts without editing the document. `view.navigate` in the
native adapter also accepts `{"motion":"roll","angle": radians,
"anchor_points":[x,y]}`: a finite signed angle in ±π and optional finite
canvas-point coordinates bounded to ±1,000,000. The reply is the normal viewport
diagnostic. Rotation uses the shared camera's `roll_by`; `frame_coords` and
`place` keep the anchor fixed. This is a display-only native protocol extension,
with no new engine command, document field, or ABI symbol.

`view.touch_orbit` is a viewport presentation preference. Input is
`{"follows_viewpoint": boolean}`; the viewport diagnostic reply includes the same
`touch_orbit` object. The default is `true`: finger deltas retain their direction
for the shared screen-relative `view.navigate` orbit, with a fixed object-centre
pivot, so the visible object follows the finger. The existing `follows_viewpoint`
wire key and `SolidCraft.workspace.viewpointOrbit` preference key are retained.
`false` restores the original world-axis, view-centred finger orbit. Changing it
cancels current contacts and leaves the camera, document and history unchanged.
UIKit persists and reapplies it at session startup; the Rust adapter owns routing
and delegates all camera maths to the existing shared navigation implementation.

`view.tool_dismissal` accepts `{"cancel_on_dismiss": boolean}` and reports the
same `tool_dismissal` object in viewport diagnostics. It defaults to `true`.
UIKit persists `SolidCraft.workspace.cancelToolsOnDismiss` and reapplies it at
startup. Closing Properties (including toggling it closed or opening the browser
in its place) cancels an active modelling/assembly draft. A stationary primary
tap on empty displayed space cancels it in Rust, invalidating pending previews
before they can publish. Finger navigation, actual geometry hits, consumed or
stale contacts do not dismiss it. Preview hit checks use visible body geometry
independent of the selection filter. `input.dismissed_model_token` retains the
last dismissed token so UIKit can close only its matching form, including when
selection was already empty. Turning the preference off preserves the previous
hide-and-resume behaviour. The form's explicit Select action can still hide
Properties temporarily to collect references without cancelling the draft.

Input events may include `sketch_annotation: string` on contact begin. UIKit
supplies the annotation ID hit within the native label's full bounds; Rust
resolves it against current annotations after document/camera/revision checks.
Omission uses the viewport's 22-point fallback hit target. IDs are bounded to
256 bytes. Select and Move route primary label drags through existing sketch
preview tickets and `sketch.dimension_text`; selected-edge measurement readouts
use a bounded session-only placement cache and the same upstream layout.

Sketch `annotations` also carry the actual engine `entity` selection and an
optional `dimension_type`. Existing annotations target `Sel::SketchConstraint`;
an undimensioned selected line or round edge targets `Sel::SketchCurve` with
`length` or `radius`. The host selects that entity before the existing numeric
Begin request. Apply alone authors the dimension; existing dimensions change
their original parameter. UIKit observes a stationary primary tap while the
complete contact stream still reaches the viewport for annotation dragging.
Multi-touch, movement beyond six points, cancellation and changed display
context suppress inline entry. Reference dimensions remain read-only.

`SessionAdapter::execute(&mut self, id, params)` returns
`Result<CommandReply, AdapterError>`. `CommandReply` contains the actual command
`value`, `document_id`, engine `revision`, adapter `geometry_revision` and
`changed`. CAD semantics and history remain in `Session::execute`. The bridge
synchronizes the viewport's `Sel` values through the existing `select.set`
command before commands that may consume selection.

The C contract is [solidcraft_ipados.h](../../crates/ipados/bridge/include/solidcraft_ipados.h),
ABI 3. Checked integer handles are opaque; zero is invalid. There are at most
eight live sessions, requests are bounded to 1 MiB, and responses to 4 MiB. No
engine pointers, borrowed geometry or Rust allocations cross C. The host copies
UTF-8 JSON responses into its own memory.

| Lane | Calls | Host serialization |
| --- | --- | --- |
| Engine | `sc_request`, `sc_response_len`, `sc_response_copy` | `SolidCraft.engine` serial queue |
| Viewport | `sc_view_request`, `sc_view_response_len`, `sc_view_response_copy` | `SolidCraft.viewport` serial queue |
| Surface | `sc_surface_attach/resize/draw/suspend`, surface response len/copy | Viewport queue plus `LifecycleGate` render lock; synchronous suspension on main uses the same lock |

A lane's operation and response len/copy form one serialized transaction. Lanes
may run concurrently; a second caller must not overwrite another caller's
response. `sc_request` additionally accepts display requests for existing replay
paths; production host routing still follows the table. Surface requests never
evaluate CAD. ABI extensions and JSON field/type changes require joint review of
Rust producers, Swift decoders, header documentation and existing decoder tests.

## Threading, lifetime and cancellation

UIKit views, native layout, form state and normal host callbacks belong on main.
`MetalCanvas` synchronously copies touch values; no `UITouch`, recognizer or view
is retained as an asynchronous input sample. CAD evaluation, tessellation,
workspace reads and native file commands run on the serial engine queue. Typed
input, picking, camera navigation and drawing run on the serial viewport queue.

The bridge registry clones an `Arc<Entry>` and releases its registry lock before
work. Engine, viewport and renderer state have separate locks. Evaluation occurs
without holding the viewport or renderer lock; accepting a preview briefly
rechecks its ticket under the viewport lock. Never hold the lifecycle render lock
while evaluating CAD or waiting for the engine queue.

The host retains `CAMetalLayer` until renderer destruction or a successful
replacement attach completes. UIKit changes layer geometry on main. Surface
operations and response copying share the render lock with resign-active
suspension. Suspension disables drawing and performs a bounded GPU drain;
background-task time is not permission for Metal submissions. Zero-sized
drawables do not render. Activation may recreate surface/GPU resources while
retaining the CAD session and immutable scene.

Suspension invalidates provisional numeric/model/tool interactions and their
publication tokens. Prepared preview work must be discarded if cancellation,
selection/context change or a newer version wins before acceptance. Ordinary
commands already accepted into the serial engine path may finish during
suspension; suspension is not rollback or preemptive kernel cancellation. No
Metal submission is permitted until activation. `NativeSession` drops callbacks
after shutdown via `alive`; lifecycle-sensitive requests also check their
generation. Future services must not promise cancellation of an already running
engine command or report a completed mutation as undone.

`NativeSession.stop()` schedules destruction after queued engine work and then
the display queue. Do not destroy the handle or release its retained layer from
a file service, inspector or individual view. Do not add a second session to
support a modal form or background export without an explicit ownership design.

## Document, scene and selection revisions

The authoritative document is `SessionAdapter.session.doc`. Swift `WorkspaceState`,
browser rows, inspection DTOs and `WorkspaceCache` are read models. They cannot
become sources for saving a design or applying geometry changes.

| Identifier | Meaning and consumer obligation |
| --- | --- |
| `document_id` | Session document identity; replacement invalidates references and drafts even if numeric revisions happen to match |
| `revision` | Existing engine revision; includes changes such as selection that need not replace model geometry |
| `geometry_revision` | Adapter publication stamp; advances for document changes/replacement and relevant engine changes, including visibility/placement, while ordinary selection/view changes do not advance it |
| Request generation | Bridge/viewport publication ordering; stale work must not install over newer work |
| `viewport_epoch` | Drawable metrics context recorded on presented frames; rejects stale layout input |
| `camera_epoch` | Camera context recorded on presented frames; rejects starting or committing edits against a stale view |
| Scene/guide/highlight revisions | Rendering invalidation keys; they are not CAD document versions |
| Tool interaction/version or model token/version | Provisional work identity; recheck together with document/geometry context before acceptance |

`Viewport::begin_update` disables picking while current geometry is being
computed. `Viewport::install` checks its generation and clears topology selection
when document/geometry changes. A controlled evaluation failure retains the
previous visible scene but must not present its picking data as current. Selection
is conservatively invalidated by regeneration, undo/redo, visibility changes,
deletion or document replacement; persistent face/edge indices are not promised.

The viewport owns display selection as native `Sel` values. Browser selection
uses `NativeSession.selectEntity` and `select.entity`, the same viewport selection
path as hit testing. The engine mirrors selection through `select.set` before
CAD actions. `WorkspaceEntity` preserves the existing schema; its text readout
is a label, not an identity to parse back into commands.

Finished sketch regions use the existing `Sel::Profile { sketch, index }` schema.
The engine worker snapshots only visible native regions and prepares their fill
and boundaries. Canvas selection and `select.entity` validate against that same
snapshot. Modelling forms capture the exact selected regions through the existing
`capture_profile_selection` API before issuing an engine command; a canvas
reselection refreshes the profile while preserving numeric inputs.

`NativeSession.onWorkspace` delivers immutable decoded `WorkspaceState` values
on main. `refreshWorkspace` coalesces requests; normal camera changes do not
require rebuilding the document tree. `onFrame` reports presentation success and
the geometry, viewport, camera and document stamps. `FoundationController`
updates `MetalCanvas` stamps only after a presented frame whose metrics match
the current drawable. An attempted draw or a freshly evaluated scene is not
evidence that the user has seen it.

## Coordinates, transforms and rendering

- Input positions are canvas-local display points, top-left origin and downward
  Y. Drawable pixels equal points multiplied by scale exactly once. The adapter's
  embedding-origin support applies `(points - origin) * scale`; the bundled
  canvas already supplies local positions. Safe-area and toolbar offsets must
  not be added again.
- `ViewportMetrics` carries point dimensions, scale and the supported embedding
  origin. Native drawable dimensions are pixels. Layout/scale changes cancel
  uncertain contacts and wait for matching presentation before accepting input.
- World geometry and length values use the engine's millimetre conventions.
  Sketch-local coordinates resolve through the native `Plane`; ray construction
  and intersections use shared `Camera::ray` and `Plane::intersect_ray` APIs.
  Do not infer an XY plane from screen orientation or flatten attached sketches.
- Camera mutation stays in `Viewport`, delegating to the existing public
  `navigate_camera` implementation and shared standard-view/bounds operations.
  Native navigation must not duplicate orbit/pan/zoom mathematics in Swift.
- `SceneSnapshot` builds visible world geometry and `PickScene` from the same
  evaluated state. `NativeMeshInstance` retains a source mesh `Arc` and its
  complete effective native `doc::Mat` from `mesh_in_place`. The renderer must
  apply that transform exactly once; it must not independently reconstruct
  component/joint placement.
- Occurrence identity includes the complete native occurrence path. Joint
  frames come from native `origin_world`. Display handles and residual feedback
  reflect the engine result, not a separate pose graph. Ambiguous repeated
  nested endpoints remain rejected where the upstream joint representation
  cannot express them.

`RenderSnapshot` retains the immutable model, display camera/projection and
guide/highlight scenes. GPU model reuse keys on the retained actual scene `Arc`,
including preview scenes; geometry revision alone cannot distinguish multiple
previews. Separate guide/highlight keys avoid full model uploads for navigation
or selection. The bounded source-mesh cache retains Arcs and monotonic keys;
raw pointer addresses without retained ownership are not stable cache identity.
The shared renderer owns depth conversion and pipelines. No idle render timer
is currently installed: input, resize, command completion and bounded retries
request frames.

## Preview, Apply, Cancel and undo/redo

There is one authoritative undo history: upstream `Session`. Native Undo/Redo
invoke `edit.undo`/`edit.redo`; UIKit form state is not an undo stack. Cancellation
discards provisional state without issuing Undo or restoring an old document over
unrelated edits.

`MetalCanvas.onUndo` and `onRedo` are main-thread shortcut callbacks. The
existing `WorkspaceController` routes them through the same commands only while
active and while the matching history button is enabled. Direct-touch double
taps do not own a separate history; text fields retain UIKit editing behavior.
`SolidCraft.workspace.historyGestures` restores the prior canvas behavior when
disabled. The integration owner owns this input contract; presentation changes
must preserve its command and availability guards.

`ToolEvent` carries Begin/Update/Suspend/Resume/Commit/Cancel and actual world
points on the tool plane. `ToolWork` carries interaction/version, source
document/geometry, `ToolOperation` and commit intent. The viewport holds one
replaceable pending work item; the host permits one queued/running tool job.
The native input mailbox is bounded to 128 samples. At capacity it thins redundant
actual Pencil moves along the same screen path while retaining contact and context
boundaries and reversals; uncompressible overload still cancels the interaction.
`InputSample` and `InputMailbox` reside in Foundation-only `WorkspaceLayout.swift`
for deterministic host checks. Their wire schema is unchanged. Predicted samples cannot commit geometry. Contact
ownership and navigation suspension/rebasing remain in `input.rs` and
`sketch_input.rs`; see [INPUT.md](INPUT.md).

`SessionAdapter::prepare_tool`, `prepare_numeric` and `prepare_model` evaluate on
upstream scratch sessions. Prepared transactions use `ComputedCommands`; the
corresponding accept methods ultimately use the existing `accept_commands`
contract after context checks. No scratch document replaces the authoritative
session merely to show a preview.

Native requests use `requestNumeric` or `requestModelling`:

- Begin returns a token and current native form/parameter data. Numeric actions
  use preview/commit/cancel; model actions also include inspect and selection.
- Extrusion selection accepts `params:{"field":"profile"|"targets"}`. Target
  mode temporarily picks whole bodies and updates only the target list; source
  mode resolves sketch regions or planar faces. The native form synchronizes
  entered parameters before selection starts. Temporary field ownership ends
  when a validated preview publishes or the operation is cancelled. The native
  viewport preference `view.automatic_subtract {"enabled":true|false}` controls
  inward-drag operation inference. `_auto_subtract` and `_auto_subtract_scope`
  are provisional form metadata stripped before shared engine commands.
- Preview validates the exact current parameters. Typing, changing selection or
  starting a newer proposal invalidates confirmation until its successful reply.
- Apply commits one logical history action. A consumed token cannot apply twice.
  Editing an existing feature preserves its original native feature ID.
- A model's `last_preview` may remain visible while newer work is pending, with
  `preview_stale` feedback. That scene is display-only and never validates the
  current parameters. Cancel or context change discards it.
- Model/assembly Pencil lift leaves a provisional preview for explicit Apply.
  The original fixture radius and sketch tools retain their documented completion
  contracts; do not impose one new completion policy across all tools.

Feature/profile/topology checks in `references.rs` and
`assembly_references.rs` precede acceptance. Keep expression text, native IDs,
component references and full occurrence paths intact. Labels, array positions
and generated display names are not replacements for native references.

## Native document I/O, errors and progress

`ProjectLibrary` organises native containers under Application Support at
`SolidCraft/Projects/<UUID>/<name>.solidcraft`. It owns filenames, directory
identity and native file access only. The UUID prevents collisions between
equally named projects. The prior `SolidCraft/Modelling.solidcraft` slot is
migrated by opening/saving a new library project; the original is retained.
`NativeSession` serialises saves and project switches on the engine queue,
debounces autosave and remembers the selected project. A launch preference can
retain the former starter-model behaviour. Diagnostic `--*-check` launches use
only their disposable fixture and never restore/autosave the library.

Two engine-lane JSON requests extend ABI 3 without changing C signatures:

- `{kind:"document", source?:path, name, destination:path}` prepares a separate
  candidate with `doc.open` or `file.new`, validates a scene and saves with
  `file.save_as`, then replaces the authoritative session. Failures preserve its
  document, undo history and revision. Native geometry revisions stay monotonic.
- `{kind:"save_document", path}` calls `file.save` without viewport publication,
  so autosave persists the committed document without cancelling numeric/Pencil
  work or publishing its provisional geometry.

The Files picker returns a URL whose security scope remains active during an
`NSFileCoordinator` read and bounded local copy on the engine queue. New-project
imports use the transaction above; insertion uses `file.insert_step` or
`file.insert_mesh`, preserving upstream component and undo semantics. Native
containers, STEP/STP, STL, OBJ and 3MF are accepted (up to 512 MiB); originals
remain unchanged. Parse/evaluation errors and supported warnings reach the UI.
Native containers with unavailable linked resources can be refused by the normal
upstream evaluator; importing additional resource files is not implemented.

Export supports native containers, STEP, STL, OBJ and 3MF. It saves first, then
freezes a local container beside the project to preserve relative-link bases.
A separate worker and disposable read-only bridge session run `file.export`;
CAD evaluation/navigation on the live session need not wait for that export.
Native-container export copies the saved file. The system export picker copies
the resulting file to a user-selected destination; completion/cancellation
cleans temporary output. No external save URL/bookmark or parallel design
format is introduced. Native serialization, import/export semantics and safe
writes remain the upstream engine/I/O implementation.

Rust failures use `AdapterError { code, message }`; the C status is 0 for success,
-1 for failure with a JSON error response, or -2 for an invalid handle. The host
uses `BridgeError`; namespaced message keys resolve through `NativeText`, and
ordinary diagnostic messages remain visible. A boundary panic marks the handle
failed; destroy/recreate it rather than continuing ordinary work. Errors must
not be converted into empty success replies or fabricated geometry.

`NativeSession.onStatus` reports activity, with a delayed tool-work indicator;
it is not a measured percentage or a promise that kernel work is cancellable.
Command completions and `onFailure` report errors; form-specific methods return
validation failures to the current draft. Preserve native assembly `reply` and
`feedback` after completion so clamps, no-motion results and solver diagnostics
remain visible. Do not treat a successful command with no movement as proof that
the requested pose was achieved.

## Shared-file ownership for parallel work

The integration owner controls changes to these shared boundaries:

- `apps/solidcraft-ipados/SolidCraft/Application.swift` and `NativeSession.swift`.
- `apps/solidcraft-ipados/SolidCraft.xcodeproj/project.pbxproj`,
  `Foundation.xcconfig`, and shared build-script contracts.
- `crates/ipados/session/src/lib.rs`, `input.rs`, `viewport.rs`, `editing.rs`,
  `workspace.rs`, `modelling.rs`, and shared reference/transaction schemas.
- `crates/ipados/bridge/src/ffi.rs`, `src/lib.rs`, and
  `include/solidcraft_ipados.h`.
- Cargo manifests/lockfile, root/scoped AGENTS files, and any upstream-controlled
  production crate changes.

Task A should own its new native document-host/service files and file-specific
tests/docs. Task B should own `bridge/src/renderer.rs`, rendering evidence and
performance work. Task C should own UI presentation files such as
`WorkspaceComponents.swift`; assign `WorkspaceController.swift` to exactly one
owner and route document-wiring edits through integration review. Task D should
own existing regression test modules, CI and verification documentation.

`WorkspaceLayout.swift` currently combines layout/focus policy with shared JSON
DTOs and `ModellingPreviewQueue`. UI ownership does not grant permission to alter
those schemas or transaction semantics. Similarly, `NativeSession.swift` holds
transport, file access, render scheduling and DTOs in one file: A/B/C must not
edit it independently. A shared-file change is proposed to the integration owner
with affected consumers, schema/lifetime impact and tests before dependent work
lands. The final exact allocation is recorded in PARALLEL-DEVELOPMENT.md.

## Feature selection feedback and history position

`SceneSnapshot` privately retains immutable `FeatureFeedback` keyed by existing
feature IDs. The engine lane projects created/changed body definitions from the
evaluator's retained before/after states, then maps their component/definition
identities to visible world bodies. It also carries solved sketch polylines and
construction geometry. The display lane resolves `Sel::Feature` against this
same snapshot for body tint/edge and non-solid feedback; it never evaluates the
timeline. Replacing the snapshot invalidates selection through the existing
revision path. These private fields add no command, ABI version or saved format.
The existing modelling projection additionally exposes `position`, the number of
features active at the shared document marker. Swift decodes it optionally, with
the end of the timeline as the fallback for older payloads. `timeline.roll_to`
runs on the engine lane and publishes a revisioned snapshot through the existing
command path; the display lane never moves the marker independently. UIKit uses
the existing `modelling.timeline`, `select.entity` and `timeline.roll_to` commands;
expansion is disposable presentation state. A future-feature tap waits for the
roll command to finish before selecting it. Native edits and future regeneration
continue through the upstream evaluator and original feature identities.
