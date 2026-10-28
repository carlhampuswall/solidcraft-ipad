# Sketching (Step 6)

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

Create Sketch opens the datum/construction/selected-face choices. Selecting a
plane previews its resolved world plane and planar camera; **Create on this
plane** commits the original native plane reference. Cancel returns the previous
view. Selecting an existing sketch in Model exposes **Edit sketch**. The active
sketch name, actual solver state and native closed-profile count appear in
Properties; **Finish sketch** remains available for open or under-constrained
geometry. **Delete sketch** is a separate explicit action with a warning that
native `timeline.delete` also removes dependent features.

Pencil places points; fingers retain pan/zoom in the editing plane. A first tap
places an anchor, a second placement completes a line/rectangle/circle. A drag
from an unanchored first point can complete the two-point primitive on release.
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

**Dimension** opens the shared expression transaction for the chosen supported
type. The initial expression is retained, units and parameter references are
parsed upstream, and Preview/Apply/Cancel have the Step 5 contract. **Place
dimension with Pencil** instead places a measured dimension at a Pencil-selected
annotation location; select it and **Edit driving expression** for a precise
value. Move / handles drags native points/curves or dimension text. Annotation
movement uses `sketch.dimension_text`, independently of its value. Reference
dimensions have no driving-expression button.

Snapping and automatic relationships have independent Tools-menu switches;
Shift suppresses both for a placement. Current target and whether it represents
a persistent-relationship proposal or a temporary positioning aid appear in the
status area. Native adapters reuse geometry intersections and stable point/curve
references, and derive the snap radius from display points. Rectangle midpoint
and intersection positioning is temporary because the rectangle creator accepts
coordinates; origin/existing-point connection still uses native `connect`.

After the core tools, the same palette exposes Trim and Extend. Selected
entities also offer construction conversion, Translate, Translate a copy,
Rotate, Rotate a copy and Mirror. Translation/rotation forms reuse the shared
expression fields, delegate lengths/angles to `expr.evaluate`, and send one
`sketch.move` through the existing provisional work mailbox for Preview/Apply.
Mirror chooses a real sketch line ID as its axis and uses `sketch.mirror`.
Native constraint/reference effects are retained; no platform topology editing
is performed.

## Verification integration

`--sketch-check` is an opt-in DEBUG device smoke driver. It waits for presented
frames and command/numeric completions instead of sleeps. It uses synthetic
Pencil events through the native input boundary to draw a real 40 × 30 rectangle,
adds driving dimensions through the shared numeric API, finishes/reopens,
changes width to 60, undoes/redoes, and checks native profile area 1,800 mm² plus
the retained expression. It leaves the real UIKit dimension inspector visible.
This driver is software injection, not a test of physical Pencil hardware or
human UI taps. Its actual run result, build/gate results, volume/serialization
acceptance and device evidence must be recorded after execution in HANDOFF.md.
