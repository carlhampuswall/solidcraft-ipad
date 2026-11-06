# Sketching (Step 6)

## Constraint colours

While editing a sketch, fully constrained curves use emerald green: RGB
`0, 125, 85` in light appearance and `70, 225, 155` in dark appearance. Free
curves remain blue/cyan, including free curves in a partly constrained sketch.
The Y axis retains its distinct yellow-green `60, 170, 60`. Finished sketches,
locked geometry, construction geometry and projected references retain their
existing styling. The shared engine still determines each curve's constraint
state; the native adapter supplies the palette through the existing scene API.

Changes a default: yes — fully constrained curves turn green. Disable **Green
fully constrained geometry** in the sketch More menu to restore the old
black/light or cyan/dark appearance. The preference is saved as
`SolidCraft.workspace.constrainedGreen`; `view.background` accepts the matching
boolean `constrained_green` without changing CAD geometry or history.

The bridge regression covers mixed constrained/free geometry, appearance and
preference changes, undo/redo and malformed input. The `render_proof`
`--constrained-green` mode captures both appearances and the legacy preference
through the actual native GPU model path, checking for distinct constrained,
free and Y-axis pixel colours. This is macOS offscreen evidence; it does not
verify UIKit runtime or physical iPad input.

## Repair a sketch attachment

After finishing or cancelling the current tool, select a sketch in the browser
and choose **Repair sketch attachment** in Properties, or hold its browser row
and choose the same action. Tap the intended planar face directly in the model.
The form stays visible and reports **Face selected** after the pick; it never
offers a list of internal face names. Use **Change face** to clear the selection
and pick again. Only bodies preceding the sketch are available, so a later
extrusion cannot become its own supporting geometry.
Face picking replaces the previous choice even when Multiple selection is on.

The repair form previews the existing sketch and regenerated model through
`sketch.redefine`. Check position and orientation, then **Apply**. Opening the
form, selecting a face and previewing do not change the saved document. Cancel
discards the proposal; Apply preserves the sketch feature ID and history marker
and adds one undoable change. The engine retains ownership of geometry,
dimensions, face certificates and author occurrence paths. Warnings and failed
downstream features still block Apply and must be repaired in history order.

Use this when a sketch reports that its supporting face changed after an earlier
sketch edit. Re-selecting an extrusion profile alone does not repair the sketch's
face attachment. Once the attachment is repaired, retry the extrusion.

The Debug-only `--sketch-attachment-check` exercises the production Properties
and browser actions, visual face picking through synthetic Pencil input,
preview, Cancel, Apply, undo/redo and an 18.8 mm
extrusion on a disposable changed-face fixture. It writes
`sketch-attachment-check.json` and `attachment-*.png` to Documents. It does not
load or autosave a library project. Reserve the physical iPad before running or
retrieving evidence. Software-driven UIKit actions do not verify physical Pencil
delivery. Focused regressions live in the existing session and bridge test files;
the engine test suite requires an explicit user request.

The earlier face-list flow was verified on 2026-10-31 with signed candidate
`a2971510ad7b0585123129310568c5246697ade3`, including the separately committed
constrained-colour work. The physical-iPad diagnostic passed Properties/browser
entry, initially disabled Apply, the keyboard-free face chooser, preview,
Cancel without changing the attachment, Apply, undo/redo and a subsequent
18.8 mm extrusion. All six UIKit/Metal captures were retrieved and inspected.
The accepted report, screenshots, binary hashes and reservation evidence are in
ignored `target/sketch-attachment-device/`; `device-refresh-acceptance.json`
records the candidate and untouched library-project scope. Normal app launch
was restored and the iPad reservation released.

The completed adapter batch passed 33 bridge and 243 session tests; the final
native-JSON coordinate fix additionally reran all three attachment repair
regressions. The native 692-case/icon harness, affected Clippy, formatting,
wasm checks, lists/i18n and unsigned/signed native builds passed. The wasm check
retains the existing engine dead-code warning. Device coverage uses programmatic
UIKit actions; physical Pencil/finger input, simulators, the engine suite and
full CI were not run for this repair. The native face payload is recanonicalised
after typed-coordinate comparison so Swift's `10.0` → `10` JSON round trip does
not reject an unchanged face. That earlier diagnostic did not exercise viewport
face picking. The replacement visual picker coalesces selection updates during
preview/context work and rejects a preview whose selected face changed before
publication; the native edit token still rejects a changed document.

The viewport-only replacement was installed and verified on 2026-10-31 with
signed candidate `5de0c271026d2d9b77b77a3895a5b5ba3b851b81`. The disposable
physical-iPad diagnostic passed all repair stages using synthetic Pencil taps,
including preview, Cancel, Apply, undo/redo and the 18.8 mm extrusion. All six
native UIKit/Metal captures were inspected. A separate direct Device Hub tap
on the model's top face, immediately after opening repair, highlighted that face
and enabled Apply after preview. **Change face** cleared the choice and disabled
Apply; another viewport tap restored a valid preview. That extra proposal was
cancelled. No face list or keyboard appeared.

This batch passed 244 session and 33 bridge tests, the 692-case/icon native
harness, affected Clippy, formatting, wasm, lists/i18n and unsigned/signed builds.
The existing engine dead-code warning remains on wasm. Evidence and binary hashes
are in ignored `target/visual-face-device/acceptance.json`, alongside the report,
captures and reservation evidence. Normal launch on the user's stand project was
restored without applying a repair to it; the iPad was released. Remote viewer
input and synthetic Pencil events do not verify physical Pencil/finger delivery.
No simulator, engine test suite or full CI was run.

## Construction geometry

Tap **Construction** before drawing to create construction lines, rectangles,
circles, arcs and splines. The highlighted toolbar button stays on while
switching tools; tap it again for ordinary profile geometry. This changes only
new geometry and resets when leaving the sketch. To convert existing curves,
use Construction in Properties or hold the toolbar button and choose
**Toggle construction for selected curves**.

Construction strokes remain orange and are dashed in previews and committed
sketches. Dash spacing follows zoom, while selection and snapping use the whole
curve. **Tools > Dashed construction geometry** restores solid strokes when
disabled; the native preference is `SolidCraft.workspace.constructionDashes`
(default enabled). CAD commands and saved sketches retain the existing engine
construction flag and format.

Construction verification on 2026-10-31: all 231 session and 30 bridge tests,
692 native workspace cases, icon checks, affected Clippy, Rust formatting,
the adapter wasm check and the unsigned device-target build pass. Four focused
regressions cover primitive creation, mode changes, stale previews, Undo/Redo,
dash phase/spacing, unbroken picking and the solid-stroke preference.
`render_proof -- /tmp/solidcraft-construction.png --construction` captures the
real GPU output after synthetic Pencil placement; the dashed and solid
preference captures were inspected. UIKit runtime and physical Pencil input
were not tested; the engine test suite was not run.

Choose **Point** in the sketch tools menu or compact tool rail, then tap to
place standalone points. Each tap is one undo step; the tool stays active for
additional points. Dragging previews the point and lifting commits its final
position. Cancel discards only the provisional point. Points are drawn as small
crosses and support selection, movement, deletion and dimensions through the
existing engine commands.

To put a point on a circle, select the point and the circle edge, then choose
**Coincident**, or choose Coincident first and tap those two entities in either
order. The existing solver creates a point-on-curve constraint. With snapping
and automatic constraints enabled, placing a point near a circle edge also
creates that relationship directly. Disabling automatic constraints keeps the
snapped position without adding the relationship.

Point verification on 2026-10-31: all 217 session and 30 bridge tests pass,
including six point regressions covering placement on XY/XZ/YZ, visibility,
selection, deletion, preview/cancel, Undo/Redo, native round trips, point snaps
and circle coincidence in both pick orders. The native workspace harness passes
692 cases and canonical icon checks; affected Clippy and the unsigned device
build pass. `render_proof -- /tmp/solidcraft-sketch-points.png --sketch-points`
captures creation, coincidence and Undo through synthetic Pencil input and the
actual macOS GPU renderer. Captures were inspected; this does not verify UIKit
runtime or physical Pencil input. The engine test suite was not run.

While drawing a line chain, a small checkmark button appears beside the last
committed endpoint. It switches to Select through `tool.set`, leaving the
committed lines and the sketch open. Choose Line again to start a new chain.
The toolbar's Finish chain button also returns to Select for lines; splines
retain their existing `tool.finish_chain` completion. The endpoint button uses the shared
camera projection, avoids workspace controls and hides during drawing/evaluation. Its accessible
label is **Finish chain** and its touch target is 44 points. The toolbar's
existing Finish chain and Finish sketch controls remain available.

DEBUG `--line-finish-check` draws two lines through synthetic Pencil samples,
checks the production UIKit endpoint button and taps it, verifies Select is
active while both lines and the open sketch remain, then draws another line
and checks the toolbar's Finish chain action also returns to Select. It writes
`line-finish-check.json` and `line-finish-ready.png`, `line-finish-finished.png`
and `line-finish-toolbar-finished.png` in Documents. Physical input
delivery requires a separate manual check.

## Capability inventory at the checked-out upstream revision

The generic `SessionAdapter::execute` already reaches all commands below. “Adapter”
means native presentation, hit testing and tool-state work is required; it does
not mean missing engine support. Command contracts are in
`crates/engine/src/cmd/sketch*.rs`, `inspect.rs` and `parameters.rs`.

| Workflow | Actual binding / important parameters | Initial platform status |
| --- | --- | --- |
| Create / reopen / finish | `sketch.create {plane,name?}`, `sketch.edit {sketch}`, `sketch.finish {}` | Engine accessible; native lifecycle adapter required |
| Planes / attached faces | `plane`: `XY`, `XZ`, `YZ`, named construction plane/id, `{origin,x_dir,y_dir}`, or `{face:[world x,y,z],body?,index?,path?}`; face attachment must have certified unique source; `fixed:true` explicitly freezes it | Native picker/preview required; retain resulting PlaneRef |
| Line chain / closure | `sketch.line {points:[point-or-ref,…],closed?,infer?,connect?}`; refs include `l1.end`; replies contain real curve IDs | Adapter required; shared point refs/connect create real relationships |
| Rectangle | `sketch.rectangle.two_point {p0,p1,connect?,construction?}` → sides and corners | Adapter required; native H/V relationships |
| Circle | `sketch.circle.center {center,radius|diameter,through?,connect?}` | Adapter required |
| Solver-aware handles | `sketch.drag {entity,from,to,with?,live?}`, `sketch.drag_end {constraint?}`; `sketch.move_point {point,to}` | Adapter required; live drag defers downstream evaluation until end |
| Geometric constraints | `sketch.constraint.coincident|parallel|perpendicular|tangent|equal|concentric {a,b}`; `.horizontal_vertical {line,mode}` or `{points,mode}`; `.fix {entity|entities,fixed?}` | Adapter eligibility/inspection required; engine validation authoritative |
| Remove geometry / constraints | `sketch.delete {entities:[ids]}` | Accessible; selection UI required |
| Delete whole sketch | `timeline.delete {feature}` (also deletes dependent features) | Explicit action only; never used for tool cancellation |
| Driving dimensions | `sketch.dimension {entities:[refs],type,value?,text_at?,driven?,if_redundant?}`; types include length, horizontal, vertical, radius, diameter, angle, distance, arc_length, sweep | Native placement/editor adapter required; engine validates units/expressions and redundancy |
| Dimension expression / annotation | `parameters.change {name,expression}`; `sketch.dimension_text {dimension,at|reset}`; `sketch.toggle_driven {constraint,driven?}` | Generalise Step 5 numeric transaction to parameter target |
| Atomic typed creation | `sketch.draw.dimensioned {command,params,dimensions}`: line length/angle, rectangle width/height, circle radius/diameter, centre arc radius/sweep | Accessible; one primitive plus dimensions is one undo unit |
| Snap / diagnostics / profiles | `sketch.snap {at,from?,radius}`; `sketch.glyphs {sketch?}`; `sketch.solve {}`; `sketch.inspect {sketch?}` | Accessible; native screen/plane adapter required |
| Arc | `sketch.arc.three_point {start,end,through}`; `sketch.arc.center_point {center,start,end|sweep}` | Engine supported; native tool required |
| Spline | `sketch.spline.fit_point {points,periodic?}`; `.control_point {points}`; `.control_point_5 {points}`; `.insert_point {curve,at,end?}` | Engine supported; native tool required |
| Trim / extend | `sketch.trim {curve,at}`, `sketch.trim.path {points,…}`, `sketch.extend {curve,at}` | Engine supported; native tool required |
| Construction / transforms | `sketch.construction {curves,value?}`; `sketch.move {entities,translate?,angle?,center?,copy?,rotate_constraints?}`; `sketch.transform {entities,factor?,angle?,center?,translate?}`; `sketch.mirror {entities,line}` | Engine supported; native controls required |
| History / native format | `edit.undo`, `edit.redo`; existing document serialisation/open/save APIs | Accessible; no format change needed |

## Reuse and limits

`sketch.snap` offers origin, existing points (endpoints/centres), line midpoints,
on-curve projection and horizontal/vertical alignment with the previous point.
Its radius is in sketch mm: the native adapter must derive it from display-point
tolerance using the active plane and camera. This command does not expose axis or
intersection targets. Richer inference/drag helpers exist in `ui-egui` but couple
to `SolveApp`, egui painting and thread-local UI state; importing that frontend or
copying its modules is not appropriate for the native adapter. Geometry
intersection itself is supported upstream.

Snap positions alone are temporary aids. Persistent relationships require point
refs (`origin`, endpoint/centre IDs, `mid:<line>`, `on:<curve>:x,y`), `connect`,
`infer`, or explicit constraint commands. Native `SketchView::no_snap` and input
modifiers provide suppression. Constraint commands detect some duplicates and
redundancy; failed strict proposals restore the previous valid document.

Use the evaluated world sketch plane (`Session::world_state`) and shared
`Plane::intersect_ray` / `to_local` / `to_world`, including component placement.
`sketch.inspect` includes world points but its returned `plane` is the authored
plane, so it alone must not replace the world-state plane for input.

`SolveReport` has `Solved` or `Failed`, actual `dof`, failing/folded/redundant IDs,
max residual and oversized-component information. A failed solve can mean
conflict or inability to solve; do not classify every failure as a proven
conflict. Fully constrained requires a successful report and zero freedom.
`sketch.solve`/`inspect` expose status/dof/failing; `glyphs` adds anchors and
redundancy. Use full report data for richer errors. `sketch.inspect.profiles`
contains actual closed profile regions/holes; under-constrained/open sketches
can still finish.

`sketch.cancel` restores the *entire editing session* (and removes a newly created
sketch), so it is not the Cancel Segment action. Tool cancellation must discard
only adapter scratch previews. Step 6 extends the existing `ToolWork` with a
native command-list operation beside the Step 4 radius operation. Its same
interaction/version/document ticket, latest-work mailbox and acceptance check
remain in force; no second session owns the document. Scratch previews and `ComputedCommands` /
`Session::accept_commands` preserve existing history and reject stale source
state. One completed primitive/segment, drag, constraint or confirmed dimension
edit is one history action; camera, predictions and cancellations add none.

Control-point spline insertion is explicitly unsupported by the engine;
fit-point insertion is supported. No other requested primary capability was
found missing in the engine during this inventory. Missing native adapters are
implementation work, not upstream limitations.

## Verification

On 2026-10-28, the full adapter run passed **59 session tests**, including 18
sketch-specific tests, plus **15 bridge tests**. These are actual engine and
deterministic input-boundary tests, not human UI or Pencil tests:

| Acceptance | Verified result |
| --- | --- |
| Dimensioned rectangle / downstream | Native origin connection and H/V relationships; 40 × 30 closed profile, zero native freedom after dimensions; 10 mm native extrusion measures 12,000 mm³. Reopen/change width to 60 retains height and relationships and measures 18,000 mm³, each within 1e-6 mm³; undo/redo restores both. |
| Native format | Actual in-memory `.solidcraft` container writer/reader round trip; reopen and edit returns the prism to 12,000 mm³. An unexposed construction ellipse retains native entity/point IDs, geometry and attributes. |
| Constraints / controlled failure | Solver drag retains horizontal relationship; one completed drag is one undo unit. A genuinely contradictory edit of a 30 mm dimension while its endpoints are fixed is refused without changing its expression, document or history; unfixing permits recovery. Duplicate relationship proposals use native `if_redundant:refuse`. |
| Curves | Separate native circle, analytic three-point arc and fit-point spline fixtures: scratch creation/cancel, native point-handle movement, undo/redo; circle driving radius. |
| Precision / focus | Stored `diameter / 2` expression survives opening; invalid/incomplete/unit-incompatible text and degenerate radius are refused; preview/cancel and lost editor token preserve the valid document. Fully determined circle drag returns visible `fixed_geometry` feedback and preserves constraints. |
| Input / planes | Synthetic Pencil line chain closes a native profile; rectangle single commit/cancel; curves and stale previews; dimension placement/text drag; non-world plane, resizing, finger navigation during Pencil work, and prior-tool/plane-preview restoration. |
| Secondary operations | Native trim/extend and transform proposal tests prove one commit/undo and cancelled-preview isolation. |

Primitive previews roll only their scratch timeline through the active sketch;
native `sketch.drag {live:true}` uses its existing bounded sketch solve and keeps
downstream results stale until commit. Completed operations regenerate the
original full timeline. Native geometric glyph queries are skipped on stale live
drag frames because that read command would otherwise regenerate the suffix;
commit restores the authoritative glyphs. There is no measured latency claim.

`parameters.change` permits a general zero distance even for a radial parameter.
For radial edits, the adapter additionally checks the evaluated radius with the
public native `Sketch::add_circle` constructor: its own validation/tolerance is
the authority. There is no platform radius threshold, clamp or solver change.

Physical Pencil, UIKit interaction and screenshots require separate device
verification. See HANDOFF.md for the actual build/device/gate results.

## Native controls and gesture grammar

Create Sketch starts a 3D plane-picking mode. Tap a translucent XY/XZ/YZ or
evaluated construction plane, or a planar body face; finger drags still orbit.
Plane labels are also native accessible selection buttons. Preselecting a plane
before Create Sketch uses that plane directly. Selecting a
plane previews its resolved world plane and planar camera; **Create on this
plane** commits the original native plane reference. Cancel returns the previous
view. Selecting an existing sketch in Model exposes **Edit sketch**. The active
sketch name, actual solver state and native closed-profile count appear in
Properties; the floating document header has a labelled **Finish sketch** button
that stays visible while the tool rail scrolls, including for open or
under-constrained geometry. The text workspace retains its labelled rail action.
**Delete sketch** is a separate explicit action with a warning that
native `timeline.delete` also removes dependent features.

**Views > Visibility** groups Grid lines, Origin point, X/Y/Z axes and XY/XZ/YZ
planes. Grid lines have a separate saved toggle in both 3D and sketch views.
The menu also offers Show all, Hide all and **Automatic during sketch creation**
for origin references. **Origin controls in Model** restores the optional browser
rows and their eye controls. Automatic is the default: references
appear while choosing a sketch plane and hide when the chooser is cancelled or
the sketch starts. The active sketch retains its own planar axes/grid. Explicit
Show/Hide choices override automatic visibility outside the plane picker. The
picker temporarily reveals XY/XZ/YZ even when hidden; **Show hidden planes when
creating sketches** can be turned off to retain the previous behavior. Reset to
Automatic to clear saved overrides. These native view preferences
persist in UserDefaults and do not edit the design or its undo history. The
world-space guides use the shared palette/camera and existing `Sel::Plane`,
`Sel::Axis` and origin `Sel::Vertex` identities. They hide during sketch editing
and numeric/modelling previews. Hidden references cannot be picked. The corner
orientation triad remains an orientation aid. Visible evaluated construction
planes also appear as 3D targets while choosing a sketch plane.

Changes a default: yes — origin references default to visibility only during
sketch-plane selection. **Views > Visibility > Show all origin references** retains
the previous always-visible setting. Disable **Tools > Select sketch planes in
3D** for the previous plane menu. Cancel (including Escape and lifecycle suspension) restores
the pre-chooser camera and never creates a feature. Only **Create on this plane**
executes `sketch.create` and adds one native undo step.

Native viewport commands: `view.choose_sketch_plane {reveal_hidden?:bool}` starts
the interactive picker. By default it temporarily reveals hidden XY/XZ/YZ planes
without changing saved visibility. `reveal_hidden:false` preserves explicit Hide
overrides; the native **Show hidden planes when creating sketches** preference
controls this parameter. Visibility controls used inside the picker still apply.
`view.origin {visible:bool|null,item?:O|X|Y|Z|XY|XZ|YZ,group?:axes|planes}`
updates guide visibility (`null` restores automatic visibility for the item,
group or all items; `item` and `group` are mutually exclusive);
`view.cancel_plane {}` restores the previous view. These are presentation commands,
not additions to the engine's CAD registry. Snapshot `origin` reports visibility
and choosing state; `reference_labels` carries native identities and view-local
label positions. The existing plane-preview request resolves attachments on a
scratch engine session. No shared source, dependency or file-format change.

Pencil places points; fingers retain pan/zoom in the editing plane. A first tap
places an anchor, a second placement completes a line/rectangle/circle. A drag
from an unanchored first point can complete the two-point primitive on release.
Creating a rectangle returns to Select. **Tools > Repeat rectangle creation**
keeps the rectangle tool active for the previous repeated-creation workflow.
Move events update a scratch preview, never a sampled polyline. Lines retain the
committed endpoint reference for the next segment; snap to the starting point to
close. **Finish chain** stops a line chain or commits the fit-point spline.
**Cancel current tool** / Escape discards only the current provisional operation;
it does not call `sketch.cancel` or remove committed segments. Three-point arc
prompts are start, end, through; fit-point splines collect explicit fit points.

Select geometry with Pencil, use **Multiple selection** for compatible
constraints, then Properties. The adapter supplies eligible actions from actual
entity kinds; commands remain authoritative. Properties shows related
constraints by default and can disclose the complete list. Tap a constraint row
or its marker to inspect/select it; related geometry is highlighted. Delete
selected entities/constraints calls `sketch.delete`. Geometric indicators use
bounded selection-sensitive screen-space lanes; dimension annotation labels
reflect native measurements and distinguish references with parentheses.

Free lengths and radii appear beside eligible sketch geometry without selecting
an edge first. Sizes determined by constraints or existing dimensions do not
get another free-size helper. In Select, tap a helper or an existing driving
dimension to enter an exact size in a popover anchored at the value.
Applied driving dimensions use bold values with solid outlines. Free-size
helpers use secondary text with dashed outlines, including when selected;
parenthesised reference dimensions remain read-only. Applying a helper changes
it to the driving style, and deleting or undoing the dimension restores the
helper style. VoiceOver identifies helpers as current measurements without an
applied dimension. **Tools > Distinguish applied dimensions** restores the
original caption appearance when disabled; this preference persists across launches.
**Tools > Show free dimension helpers** restores the previous selected-edge-only
helpers when disabled. Both this preference and repeated rectangle creation are
saved across launches. Helpers never open the editor automatically.
The numeric keypad opens with
the current expression selected; the keyboard switch supports units, arithmetic
and parameter names. Preview and Cancel preserve the committed sketch. Apply
creates one native dimension or edits the existing driving parameter, with one
undo step. Existing endpoint, radius and diameter dimensions are reused.
An existing driving dimension also offers **Delete dimension** in the popover,
above Cancel and Apply. Tap its value, then Delete dimension to remove only that
constraint and keep the geometry. Any uncommitted expression or preview is
discarded first; deletion is one undo step. Free-size helpers have no delete
action because they do not yet represent a saved dimension.
Parenthesised reference values remain read-only. Tapping one opens a compact
popover with the measured value, **Make driving**, and **Close**. Existing driving
dimension popovers also offer **Make reference**. Switching modes closes the
popover and converts the committed dimension in one undo step; any uncommitted
expression or preview is discarded first. Tap the value again to use its new
mode. Free-size helpers have no mode switch until a dimension is applied.
A drag continues moving the annotation rather than opening the editor.
Multi-touch navigation, cancellation, display changes and suspension cannot
turn an interrupted contact into an edit.

The popover follow-up passed the physical iPad DEBUG checks on 2026-10-31,
candidate `fd700c20`. The reference popover has no enabled input, Apply action,
numeric token or keypad; Close releases its editing context. Both popover mode
buttons, Undo/Redo and reopening passed. A 100 mm driving preview followed by an
invalid expression was discarded when making the saved 50 mm dimension a
reference. The existing inline entry/deletion check also passed, including
helpers without saved-dimension actions. Popover screenshots were visually
inspected; evidence and signed-binary hashes are in
`target/dimension-popover-device-20261031/`. Unsigned/signed builds and the
692-case native harness passed. Coverage uses synthetic UIKit actions, with
physical Pencil and VoiceOver unverified; no engine tests were run.

The DEBUG `--inline-dimension-check` launch flag prepares a disposable rectangle
for physical tap, numeric-entry and drag checks without opening a library project.
Tools > **Tap dimensions to edit** disables inline entry and restores selecting
labels without opening the editor; the native preference is saved across launches.
With `--inline-dimension-check-run` also supplied, the bounded DEBUG driver checks
real UIKit focus/selection, Apply, reopening, Preview/Cancel and Undo/Redo. This
passed on the physical iPad on 2026-10-29, alongside 144 session and 28 bridge tests,
the native layout harness, Clippy, wasm compilation and device builds. The real
iPad popover/keypad was visually inspected. The driver uses synthetic control
actions; physical tap, Pencil and VoiceOver coverage remain unverified.

To add a reference measurement, select sketch geometry, open **Properties >
Reference dimension**, and choose a dimension type. It appears immediately at
the native default annotation position and can be dragged into place. Reference
dimensions show parentheses, follow the geometry, and do not constrain it or open
the numeric editor. Select the dimension's constraint row in Properties to use
**Make reference** or **Make driving**. Making a reference driving uses
its current measurement; select its value afterwards to edit it. The engine
refuses a driving conversion that would over-constrain the sketch and preserves
the reference dimension. Creation and each conversion are independently undoable
through the existing `sketch.dimension` and `sketch.toggle_driven` commands.

Verification on 2026-10-31: the DEBUG `--reference-dimension-check` passed on a
physical iPad on candidate `5597b7cf`. Four UIKit captures were visually inspected:
the creation control, parenthesised reference with **Make driving**, driving
value with **Make reference**, and the restored reference after Undo/Redo.
The driver checks the type menu and shared creation callback, invokes the actual
toggle buttons, and verifies the final reference has no expression editor.
Evidence and signed-binary hashes are in `target/edge-radius-device-20261031/`.
This is synthetic UIKit control coverage; physical menu taps, Pencil and VoiceOver
remain unverified. The focused session regression passes for length, horizontal,
vertical, angle, radius and diameter, including parameter/DOF changes, editability,
history and atomic rejection of a redundant driving conversion. Unsigned/signed
device builds, the existing native harness, formatting, adapter Clippy, wasm and
locale checks passed. The engine test suite was not run.

**Dimension** opens the shared expression transaction for the chosen supported
type. The initial expression is retained, units and parameter references are
parsed upstream, and Preview/Apply/Cancel have the Step 5 contract. **Place
dimension with Pencil** instead places a measured dimension at a Pencil-selected
annotation location; select it and **Edit driving expression** for a precise
value. In Select or Move / handles, drag a dimension label directly with touch,
Pencil or a primary pointer. The label retains the grab offset; its extension
lines and arrows follow throughout the preview while their measured endpoints
stay attached to the sketch. Annotation movement uses `sketch.dimension_text`,
independently of its value, and commits one undo step. Cancel or navigation
discards the provisional placement. Reference dimensions support the same drag
and have no driving-expression button. A selected undimensioned edge's measured
length/radius can also be moved; its placement stays in the current session and
creates no constraint or history entry. Both readouts use upstream dimension
layout for their text, lines and arrows.

In Select, drag sketch points or edges directly with Pencil, touch or a primary
pointer. The existing `sketch.drag` solver keeps fixed geometry and driving
constraints intact, allowing only the sketch's remaining freedom. Selecting
several entities moves them as a group. A tap toggles selection; movement within
five display points creates no geometry proposal. A completed drag adds one undo
step; cancellation discards the preview. A second finger cancels a touch geometry
drag and rebases both contacts for pan/zoom. Empty-space finger drags still pan.
Disable **Tools > On-model editing handles** to restore Select's previous
selection/navigation behaviour; **Move / handles** remains available for Pencil
drags. This extends the existing saved handle preference without changing CAD
commands or the native file format.

Changes a default: yes — Select directly drags sketch geometry; the previous
behaviour remains behind **Tools > On-model editing handles**.

Snapping and automatic relationships have independent Tools-menu switches;
Shift suppresses both for a placement. Current target and whether it represents
a persistent-relationship proposal or a temporary positioning aid appear in the
status area. Native adapters reuse geometry intersections and stable point/curve
references, and derive the snap radius from display points. Rectangle midpoint
and intersection positioning is temporary because the rectangle creator accepts
coordinates; origin/existing-point connection still uses native `connect`.

The left sketch rail exposes Delete beside Trim. Delete stays active for repeated
taps on curves, including their endpoints; each removed curve uses the existing
`sketch.delete` command and is one undoable action. It never expands deletion to
other selected entities. Trim cuts a section between intersections; Delete removes
the whole curve. The sketch palette also exposes Delete, Trim and Extend. Selected
entities also offer construction conversion, Translate, Translate a copy,
Rotate, Rotate a copy and Mirror. Translation/rotation forms reuse the shared
expression fields, delegate lengths/angles to `expr.evaluate`, and send one
`sketch.move` through the existing provisional work mailbox for Preview/Apply.
Mirror chooses a real sketch line ID as its axis and uses `sketch.mirror`.
Native constraint/reference effects are retained; no platform topology editing
is performed.

## Verification integration

Origin-reference regressions are in the existing session and bridge test files:
all three plane identities through finger/Pencil/pointer input, per-item hiding,
axis/origin selection, topology-filter priority, construction visibility, stale
input, multi-finger suppression, cancellation/history and portrait framing. The
bridge regression verifies filled planes and the origin dot use the shared
guide slot's supported depth-tested surface lane, without entering CAD meshes.
`render_proof /tmp/solidcraft-origin.png --origin` captures visible, individually
hidden, selected and cancelled-preview states using that production converter
and the shared GPU. These are Mac GPU captures; native labels/chrome and physical
iPad touch interaction still need separate device verification.

`--sketch-check` is an opt-in DEBUG device smoke driver. It waits for presented
frames and command/numeric completions instead of sleeps. It uses synthetic
Pencil events through the native input boundary to draw a real 40 × 30 rectangle,
adds driving dimensions through the shared numeric API, finishes/reopens,
changes width to 60, undoes/redoes, and checks native profile area 1,800 mm² plus
the retained expression. It leaves the real UIKit dimension inspector visible.
This driver is software injection, not a test of physical Pencil hardware or
human UI taps. Its actual run result, build/gate results, volume/serialization
acceptance and device evidence must be recorded after execution in HANDOFF.md.

## Rectangle dimension helpers

Free rectangle sizes display two editable helpers by default: width above the
sketch's top edge and height outside its right edge, with a 36-point offset.
The adapter recognises closed H/V-constrained rectangular line loops once per
scene publication; it does not infer relationships between separate lines of
equal length or change CAD constraints. Existing dimensions continue to replace determined free sizes,
and dragged helper positions override the initial placement.

Rectangles with horizontal/vertical constraints use this layout automatically;
there is no separate preference for duplicate rectangle dimensions. Loops without
those relationships retain independent size helpers. The existing Show free
dimension helpers switch still restores selected-edge-only helpers.

Verified on 2026-10-31: 199 session and 28 bridge tests, Clippy with warnings
denied, Rust formatting, the native workspace harness (692 adaptive cases), and
the unsigned iPad device-target build.
`render_proof /tmp/solidcraft-rectangle-dimensions.png --rectangle-dimensions`
captures the actual shared-GPU arrows above and right of a 40 × 30 rectangle,
checks the projected helper positions, and tests hiding/restoring them. UIKit
labels and physical iPad interaction are not covered by the Mac GPU capture.

## Applying constraints before picking

The Constraints sheet allows choosing a constraint with an empty selection. The
chosen constraint remains active while tapping compatible sketch geometry with
touch, Pencil or a pointer. Horizontal, Vertical and Fix apply after one pick;
constraints such as Parallel collect two picks without requiring Multiple
selection. Existing eligible selections still apply immediately. Each successful
application is one ordinary engine transaction and undo step, and the tool stays
active for another application. Choose Select or Cancel current tool to exit.

The status names the active constraint and prompts for the next pick. Tapping
the first entity again deselects it; empty space clears pending picks. Incompatible
entities do not create a constraint. The shared engine validates the final pair,
refuses redundant constraints and reports failures without changing the design.
Constraint mode disables geometry drags and dimension editing while preserving
normal camera navigation. Finishing or replacing the sketch exits the mode.

## Referencing an earlier sketch

While editing a sketch, open **Sketch Tools > Project / Reference Geometry**.
The main viewport displays the earlier sketches as selectable reference geometry.
Tap a curve, endpoint, centre or standalone point with Pencil to choose its source
sketch and select that reference. Tap again to deselect. Point picks take priority
near endpoints and centres; the **Curves** and **Points** filters resolve ambiguity.
Two-finger navigation continues through the ordinary viewport camera path.

The options inspector displays the selected source name and reference count,
with no entity-ID selection lists. **Choose sketch** returns to visual source
picking. **Next sketch** isolates the next earlier sketch when sources overlap.
**All geometry** selects the chosen source's planar curves and standalone points;
it excludes curve-owned points and the implicit origin, which remain individually
pickable. **Clear** clears the picks. A confirmation accepts at most 1,000 entities.

**Construction** creates guides that do not form profiles; turn it off to use
projected boundaries as profiles. **Link to source** keeps references updated;
turn it off to create detached geometry. Later, select linked curves and use
**Break Link** in the inspector, or choose a reference from **Sketch Tools >
Break Link**, including standalone projected points. Detaching preserves the
current geometry and construction state.

Picks live only in the viewport until **Project**. Cancel leaves the document
unchanged. Geometry revisions and document/sketch identity invalidate stale
picks, and the engine still validates the pinned confirmation. Project plus
Construction remains one undo step. The engine supplies world-space source
geometry to the existing Rust renderer and input path; UIKit neither resolves
geometry nor implements another camera.

Focused adapter/ABI regressions use the `sketch_projection` test filter. The
native harness checks the viewport-pick request contract. The opt-in
`--sketch-projection-check` device driver exercises visual source/curve/centre
selection through synthetic Pencil events and production controls. It writes
`sketch-projection-visual-check.json` and `projection-visual-{picks,all,projected}.png`.
Run it only inside a device reservation; synthetic events do not verify physical
Pencil contact.

Historical verification of the initial sheet implementation follows; it does
not qualify the later main-viewport interaction change.

Device verification on 2026-10-31: candidate `c8602c21` passed the production
UIKit driver on the connected physical iPad. Source switching, individual/all
selection, linked construction projection, one-step undo/redo and selected-curve
Break Link passed with no reported issues. The chooser and projected-sketch
screenshots were captured and visually inspected. Evidence remains untracked in
`target/sketch-projection/` (`ipad-session.txt`, `device-report.json`,
`projection-chooser.png`, `projection-projected.png`). Physical Pencil contact
was not tested. The same implementation passed 238 session tests, 31 bridge
tests, the native deterministic harness, unsigned/signed device builds, adapter
Clippy, wasm, formatting, lists and i18n checks. The wasm build retains an
existing engine dead-code warning; the engine test suite and full CI were not
run under the task-coordination policy.
