# Native Pencil and multitouch input (Steps 4–8)

The development host retains one source-aware pipeline. The original Step 4 tool
remains available alongside the later sketch, modelling and assembly tools.
Select **Post radius**, then drag the cross at the cylindrical post's top rim.
The handle edits that fixture feature's `radius` with `timeline.edit` (0.5–50 mm).
Select remains the initial tool. Undo/Redo and Command-Z/Shift-Command-Z use engine
history; Escape or Cancel interaction discards a provisional interaction.

## Selecting a sketch

Tapping a sketch in History selects it without starting an edit. **Edit sketch**
appears below the document title, in the same place as **Finish sketch**. Tap
**Edit sketch** to enter the sketch; the button then becomes **Finish sketch**.
The same action is available for a single sketch selected in the browser.
Clearing or changing the selection updates the action. Enable **Tools → Edit
sketches on history selection** to restore immediate editing from History.

## Home and model framing

The left navigation toolbar's **Home** button restores a fixed orthographic
isometric view: target at the world origin, 200 mm eye distance, Z up and no roll.
Its location and zoom do not depend on model bounds. **View → Fit model** keeps
its existing model-centred framing and preserves the current orientation.
Home is unavailable in a planar view; finish the sketch and leave planar mode
before returning Home. Fit model remains available while editing a sketch.
Disable **Tools → Home button in toolbar** to restore the previous Fit model
button (`SolidCraft.workspace.homeToolbar`). This does not change a design.

## Gesture mapping

| Input in the viewport | Action |
| --- | --- |
| Pencil tap in Select | Visible enclosed sketch region or actual occurrence/body/face/edge/vertex/Auto picking; Multiple still applies |
| Pencil drag on the Post radius handle | Actual plane intersection → provisional solid → one command on completion |
| Pencil drag on an active modelling/assembly handle | Native parameter proposal and scratch geometry; lift leaves a preview for explicit Apply |
| Touch, Pencil or primary pointer drag on a sketch dimension label in Select or Move | Reposition text, extension lines and arrows together; measured geometry stays fixed; one undo step for a saved dimension, session-only placement for a selected-edge measurement |
| One finger | Tap empty space in Select to clear selection on release; drag to turn the visible object in 3D or pan in an active sketch plane; an enabled direct handle captures its own drag |
| Two fingers | Simultaneous centroid pan, distance-ratio zoom and twist rotation about the viewing direction |
| Two-finger double-tap | Undo one engine history action |
| Three-finger double-tap | Redo one engine history action |
| Pointer primary / hover | Existing selection / preselection |
| Secondary drag / middle or Shift-secondary | Existing orbit / pan; planar mode maps orbit to pan |
| Wheel or trackpad scroll | Shared zoom path |
| Input beginning on a native control | That control owns it until UIKit completes/cancels it |

In sketch view, two-finger twist snaps the sketch axes to every **45°**, including
0°, 90°, 180° and 270°. A detent engages within 3° and releases beyond 5°;
small movements continue to accumulate while held. Snapping is relative to the
sketch plane, including face-attached planes, and retains the gesture centroid.
Lifting/cancelling or changing finger count rebases without a camera jump.
Disable **Tools → Snap sketch view rotation** for the previous free rotation;
the choice persists as `SolidCraft.workspace.sketchRotationSnap`.

`view.sketch_rotation_snap {"enabled":true}` sets this native presentation
preference. The input requires a boolean; invalid input leaves contacts and the
preference unchanged. The output is the viewport diagnostic object, including
`sketch_rotation_snap.enabled`. Changing it cancels current input. Explicit
`view.navigate` roll commands retain their exact angles.

`render_proof --sketch-rotation-snap` sends synthetic finger events through the
native adapter and captures the shared GPU output: 88° → 90°, 43° → 45° and
−2° → 0°, alongside the free 88° setting. This is macOS render evidence;
physical finger delivery on iPad remains a separate check.

Double-click a solid edge or double-tap a sketch curve with Pencil or a primary
pointer to select all edges connected through their endpoints. Sketch curves also
support direct finger double-taps when direct handles are enabled. Corners and
branches at shared endpoints are included; T-junctions touching only a curve's
middle, crossings, centres and spline control points are excluded. Connectivity
stays within the current sketch or picked body/occurrence. The second tap expands
the selection instead of toggling off the first edge; Multiple retains unrelated
picks, while single selection replaces them with the connected group. Groups over
the existing 1,024-item selection limit are refused without a partial selection.

Disable **Tools → Double-click connected edges** to restore ordinary per-tap
selection (`SolidCraft.workspace.connectedEdgeSelection`). UIKit forwards its
`tapCount` as optional `input` field `tap_count`; only a value of 2 expands a
selection. Omitted/zero counts preserve existing replay behaviour. Normal drag,
consumed/stale input, modelling forms, dimensions and active sketch tools retain
their existing input ownership. Selection does not edit the design or add history.

`render_proof --connected-edges` captures a single selected sketch edge and the
expanded endpoint loop/branch while leaving its T-junction and crossing unselected.
These are synthetic Pencil events and actual macOS GPU captures; physical iPad
delivery remains a separate check.

Selection starts in **Auto**: a Pencil tap near a visible body edge selects the
edge, a tap near its endpoint selects the vertex, and a tap inside a face selects
the face. Edge and vertex tolerance is eight display points at every screen scale.
**Tools → Filter** still offers explicit Edge, Vertex, Face, Body and Occurrence
filters. The chosen filter persists in `SolidCraft.workspace.selectionFilter`;
select **Face** to retain the previous face-only behaviour after relaunch.

**Multiple** selection starts enabled. Successive Pencil or primary pointer taps
keep earlier edge, face and other geometry picks; tapping a selected item again
removes only that item. An ordinary tap on empty space, or **Tools → Clear**,
clears the selection. Turn off **Tools → Multiple** (also in Properties) for the
previous single-selection behaviour. This choice persists in
`SolidCraft.workspace.multipleSelection` and is restored when the session starts.
Shift still adds or removes picks while Multiple is off. Fillet and Chamfer
receive every selected edge; face operations receive every selected face where
the existing command supports it. Selection changes do not enter model history.

A single finger can also clear selection by tapping empty viewport space in
Select. Touch-down retains the feature and its highlights until release, so a
camera drag keeps the selection. Taps on geometry retain the current selection,
even when the active selection filter excludes that geometry. Two-finger gestures,
overlapping Pencil contacts, cancelled/consumed input and stale displayed frames
do not clear selection. Browser and history highlights follow the same selection;
the document, camera and undo history stay unchanged. Active modelling drafts
retain their existing empty-space cancellation and dismissal preference.

Pencil selection uses the touch-down position and tolerates up to ten display
points of contact motion, including the final lift sample. Moving toward the next
target while lifting therefore does not lose the face or edge originally tapped.
An empty-space tap uses its own touch-down position to clear selection. A stroke
that exceeds that tolerance stays a stroke even if it returns to its start.
Sketch geometry and tool handles retain their five-point drag threshold.

`render_proof --selection-taps` captures a face selection, empty-space clearing
and reselection after moving hover, with six/eight-point lift motion. These are
synthetic Pencil events and real macOS GPU captures; physical Pencil delivery
remains a separate check.

Cylinder rims split into kernel edge pieces hover and select as one complete
circle. Tapping either half again deselects the whole rim; inner, outer, top and
bottom rims remain independent. Single selection also retains all pieces of the
chosen rim, so edge tools receive the complete circle.

After finishing a sketch, tap inside its enclosed region with Pencil, then choose
Extrude. The region highlights and populates the profile field directly, including
when the sketch has several regions. Multiple selection can combine regions from
one sketch. **Select geometry** in the modelling form also accepts a new canvas
region while retaining the distance and other entered values. Face and Auto
filters accept profiles; explicit Body, Occurrence, Edge and Vertex filters retain
their existing targets. Only shown sketch profiles can be selected; regions
behind solid geometry are excluded. Picking uses the engine's world-space
profile regions, including their holes, without another solver or profile detector.

The shared GPU reproduction uses synthetic Pencil input to select one of two
regions and extrude just that 200 mm² region by 10 mm (2,000 mm³):

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-profile.png --profiles
```

It captures `-before.png`, `-selected.png` and `-extruded.png` on macOS. This is
adapter/rendering evidence; physical Pencil and UIKit verification remain separate.

One-finger orbit follows the screen axes, including rolled, top and bottom views,
and passes smoothly through the poles. Drag right to turn the visible object
right; drag down to turn it down. The viewpoint moves in the opposite direction. The orbit
pivot is the centre of the body under the initial contact, or the visible model
centre over empty space. It is fixed for the gesture and stays in place on screen,
even after panning, so the object turns around itself rather than circling an
off-centre camera target. One/two-finger transitions choose a fresh pivot without
moving the camera. Disable **Tools → Rotate around object with one finger** to restore
the previous world-axis, view-centred orbit and direction. The preference persists
in `SolidCraft.workspace.viewpointOrbit` and never changes a design. Sketch pan,
two-finger pan/zoom and pointer navigation retain their existing mapping.

The existing `render_proof` example accepts `--orbit` for a repeatable GPU
comparison of a rightward drag after panning a top view:

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-orbit.png --orbit
```

It writes `-before.png`, `-previous.png` and `-fixed.png` using the real input
router and shared renderer, and checks that the corrected object's centre does
not drift. This is macOS GPU evidence, separate from physical iPad verification.

Two-finger twisting turns the view in the same screen direction as the fingers,
including in a sketch plane, without tilting that plane. Pan and pinch remain
available during the same gesture. Rotation uses the shortest signed angle
between successive contact vectors, so crossing ±180° does not jump. The current
finger midpoint stays fixed during the rotation component. Disable **Tools →
Rotate view with two-finger twist** to restore pan/pinch without rotation; the
choice persists in `SolidCraft.workspace.twoFingerTwist` and never edits a design.
The native navigation adapter delegates rotation and anchor preservation to the
shared `Camera::roll_by`, `frame_coords` and `place` methods; no camera math is
copied into UIKit or a second controller.

History double-taps are enabled by default and can be disabled under **Tools →
Double-tap gestures for Undo/Redo** to restore the previous input behaviour.
They use the toolbar's availability checks, including pending commands and active
edits. Single taps do not change history; Pencil and pointer taps are excluded.
UIKit recognisers observe direct contacts without delaying the Rust navigation
path; movement beyond its six-point threshold cancels tap recognition. Lifecycle
and input cancellation also clear a partially completed double-tap. The canvas
disables UIKit's competing editing gestures while these shortcuts are enabled;
text fields retain their own editing gestures.

Ending a sketch or **Exit sketch view** explicitly
leaves planar navigation; standard-view/Home actions cannot silently orbit an
active planar view. Navigation delegates to the Step 3 `navigate_camera` path.

## Boundary, coordinates and ownership

`MetalCanvas` synchronously copies UIKit touches into `InputSample` values. Source
comes from `UITouch.type`: `.pencil`, `.direct`, or indirect mouse/trackpad input.
Force is never used to classify a source. Object identities stay inside UIKit;
monotonic contact IDs survive coalesced samples through end/cancel. No `UITouch`,
recognizer, UIView or borrowed geometry crosses to asynchronous Rust work.

Samples carry phase, uptime timestamp, sequence, presented document/geometry,
viewport and camera epochs, presented-preview status, display-point position, drawable-pixel position,
scale, optional force/altitude/azimuth, button/modifier and prediction status.
Mouse and trackpad share UIKit's indirect identity. Coalesced actual Pencil moves
are copied in time order (up to 32 per callback); the delivered sample is included
once. Predicted samples are deliberately not requested and are rejected by the
router, including predicted end events. Estimated force/tilt corrections are not
used for CAD dimensions. Basic editing works with all optional properties absent.

The 128-sample native mailbox thins redundant actual Pencil motion when rendering
falls behind. It discards queued hover feedback (including id-zero hover exits)
first, and drops incoming hover feedback
if a full queue contains only authoritative input. Hover cannot cancel a tap.
For contact samples it retains begin/end/cancel, reversals and context boundaries;
only intermediate moves on the same screen path (within a quarter point) can be
removed. If neither hover nor redundant motion remains, authoritative-input
overload still cancels conservatively.
The Foundation-only input values and mailbox live in `WorkspaceLayout.swift` so
the native harness exercises production batching without UIKit or a simulator.
Stationary handle samples do not supersede an in-flight geometry preview. Tapping
a handle releases capture, and a new grab supersedes the previous lifted contact's
pending preview. Preview completion still checks the current interaction ticket.
Form inspection preserves a new operation's proposal version, so refreshing its
fields after a preview does not release the live contact. During continuous
modelling movement, completed scratch geometry from the same live contact can
advance the display while the latest sample is still being evaluated. That
geometry remains visibly stale, cannot enable Apply, and never rewinds the handle
or its current parameters. Older/duplicate completions, lift, cancellation,
navigation suspension/rebase and context changes reject late progress.

The `render_proof --continuous-resize` check creates a new extrusion and captures
one synthetic Pencil contact through repeated motion, in-flight progress and
form inspection, before explicit Apply. This uses the macOS GPU; it does not
verify physical Pencil delivery or UIKit.

Points are canvas-local, origin at top left, Y downward, in display points.
Pixels equal points × scale exactly once. The adapter also supports an explicit
embedding origin: `(points - origin) × scale`. The safe-area toolbar is outside
the canvas. Input waits for a matching presented drawable after native layout;
stale viewport/document contexts cancel, and stale camera contexts cannot start
a pick/edit or commit a drag. A still-presented cancelled preview cannot start a
new pick/edit before the committed scene is presented again. Plane placement uses shared `Camera::ray` and
`Plane::intersect_ray`; picking uses Step 3's shared mesh indices/topology.

`input.rs` owns a bounded contact map with stable owners: UI, navigation, Pencil
selection candidate, tool, pointer or suppressed. A separate Pencil edit tracks
Candidate → Dragging → Committing, or Suspended → Rebase → Dragging. Contacts of
different sources coexist. A consumed begin cannot become a viewport action
later. Duplicate sequence/begin events cannot create a second action. Time reversal
or malformed samples cancel safely. UIKit's native touch path is the only contact
producer; there is no tap/pan/pinch recognizer synthesizing duplicate CAD contacts.
Hover and scroll recognizers only report their distinct non-contact phases.
History tap recognizers invoke the existing commands and never emit contacts.

Pencil movement beyond 5 display points begins an actual tool or sketch drag;
selection-only Pencil contacts use the ten-point tolerance described above. Finger
navigation requires over 6 points; while Pencil is down, one finger requires over
12 points and 80 ms. Two-finger intent requires over 6 points and at least 24
points of separation. Three or more viewport fingers produce no camera deltas.
One/two-finger transitions rebase anchors and have zero camera delta themselves.
These conservative thresholds supplement UIKit; they do not claim perfect palm
rejection and do not reject every finger while Pencil is present.

## Simultaneous input, cancellation and work

Intentional finger navigation suspends the Pencil edit and invalidates pending
preview tickets. The last actual preview remains visible; its original document
and history remain untouched. When all navigation fingers end/cancel, the first
actual Pencil move only rebases the plane anchor and current radius. Subsequent
moves resume editing. Pencil lift/cancel while suspended or awaiting rebase cancels
the edit. A stationary selection candidate is invalidated by camera navigation.
Both Pencil-first and fingers-first orders are covered by replay tests.

Pencil leaving the canvas cancels its edit; navigation keeps capture until end.
Tool changes, explicit cancellation, document updates/replacement, resize/scale
changes, background/focus loss, view removal and destruction clear active state
and discard provisional geometry. Entity/document/revision checks reject deleted,
hidden or changed handles. Cancellation never calls Undo or restores a checkpoint
over unrelated work. Pressure and tilt never change geometric dimensions.

`ToolEvent` exposes Begin/Update/Suspend/Resume/Commit/Cancel, interaction ID and
actual plane point. `ToolWork` carries the immutable command parameters and source
context. The display worker has one replaceable pending work item; native code
permits one executing/queued engine job. A bounded 128-sample native input mailbox
preserves phase boundaries normally; overload cancels the uncertain interaction.
Stale results are discarded by interaction/version and document/geometry checks.

Preview uses upstream `Session::scratch`, command evaluation/cache and scene
building off the input thread. Commit prepares `compute_commands` off-thread,
then checks the ticket under the viewport lock and calls evaluation-free
`accept_commands`. Exactly one `timeline.edit` reaches authoritative history.
The direct fixture-radius completion has this one-command contract. Later
`ToolOperation::Modelling` work is preview-only: explicit Apply commits the draft.
The shared renderer displays real evaluated preview meshes; prediction or a marker
alone is never treated as geometry evidence. No new solver, renderer or undo stack.
The fixture parameter handle has no sketch snapping/inference. Step 6 adds native
sketch controls over the shared snapping/solver APIs; no upstream frontend module
is imported into this adapter.

## Optional Pencil capabilities

A separate Pencil-only hover recognizer provides real geometry preselection on
capable devices. iPadOS 17.5+ honours `prefersHoverToolPreview`; older supported
systems use their delivered hover events. Double tap (legacy and 17.5+ delegates)
and squeeze (17.5+) switch Select/Post radius only when the system preference is
`switchPrevious`. Other preferences have no mapping to an unimplemented tool.
UIKit delivers these callbacks only for capable hardware; no model/pressure guess
is used. There is no deletion mapping, eraser, fake palette or hover requirement.

APIs were checked against the installed iPhoneOS 27 SDK with deployment target 17,
and Apple's current [input](https://developer.apple.com/documentation/uikit/handling-input-from-apple-pencil),
[coalescing](https://developer.apple.com/documentation/uikit/getting-high-fidelity-input-with-coalesced-touches)
and [interaction documentation](https://developer.apple.com/documentation/uikit/uipencilinteraction).
Compilation does not verify hardware support or latency.

## Physical-device checklist (manual checks still required)

- [ ] Pencil taps select each filter without camera movement; drags differ from taps.
- [ ] Post radius previews a solid, commits once, and Undo/Redo restore geometry.
- [ ] One/two/one finger navigation has no jumps; sketch mode stays planar.
- [ ] Two-finger double-tap undoes once; three-finger double-tap redoes once. Single taps and drags do neither.
- [ ] History gestures respect active edits/pending commands; disabling the Tools preference restores the old behaviour.
- [ ] Both source orders suspend/rebase correctly; lift during suspension cancels.
- [ ] Palm/incidental contacts do not edit; intentional two-finger navigation works.
- [ ] Control-to-canvas crossing, canvas exit, Cancel, Escape and tool change clean up.
- [ ] Rotation, resizing, focus loss, background/foreground and view removal clean up.
- [ ] Pointer/trackpad, keyboard shortcuts and ordinary native controls still work.
- [ ] Check available hover/double tap/squeeze and system preferences; record hardware.

Automated replay/engine tests, Mac GPU captures, builds and synthetic device input
are separate coverage levels. Actual observed results are in [HANDOFF.md](HANDOFF.md).

## Step 5 native workspace integration

UIKit owns toolbar, tree, inspector and scrim contacts. Wide panels are outside
the actual Metal view; compact panels block canvas input and consume outside taps.
The canvas frame drives rendering and picking metrics together. Direct contacts
cancel before layout/focus changes; reversible aspect correction uses the same
shared navigation path and never changes CAD geometry.

Numeric properties add a separate command-based transaction owner, not another
contact pipeline: selection/tool/history controls lock until Apply/Cancel.
Keyboard metric changes preserve the numeric token; suspension cancels it and
invalidates queued callbacks. Text fields keep their own undo; document shortcuts
are withheld during text/draft focus. See [UI.md](UI.md) for exact contracts.

## Steps 6–8 integration

Sketch placement/solver handles, extrusion handles and assembly movement/driving
extend the same typed contact router and bounded work mailbox. They do not add a
second gesture producer, camera, history stack or solver. Assembly selection uses
full native occurrence paths, including nested repeated placements. Scene,
world-space picker and reference validation share one document revision.

Assembly handles are proposals for the selected native parameter: supported
placement translation/rotation, joint driving and reach use the engine's actual
frames and solve. The UI offers precise expressions alongside the handle. A
Pencil lift does not capture a position or commit a joint drive automatically;
Preview/Apply/Cancel retain the Step 7 model-draft contract. Native clamping,
constraint stops and no-movement reasons remain visible. See
[ASSEMBLIES.md](ASSEMBLIES.md) for supported operation/handle limits.

Between actual Pencil samples and worker completion, one last-good pose remains
visible, explicitly marked stale. It cannot validate a newer value or enable
Apply. The latest current result replaces it; stale completions are discarded.
This held-worker interval has a deterministic regression and the rotation fix
was confirmed by the user with a physical Pencil. See HANDOFF for the distinct
human and automated runtime records.

Intentional finger navigation suspends/rebases these Pencil interactions through
the existing policy. Resize, focus loss, document changes and cancellation reject
old work; explicit draft cancellation removes provisional work only. UIKit
buttons, choices, numeric fields and sliders consume their own touches. Sliders
coalesce intermediate samples into one in-flight evaluation and one latest
proposal, restore the starting draft on interrupted input, and require explicit
Apply to affect native history.

Changing joint/type/DOF, placement or other context choices refreshes native form
controls without sending expression keystrokes. Form-version and exact-parameter
checks prevent old preview/context callbacks from replacing newer text or
enabling confirmation. Finger/Pencil viewport routing remains available while
context inspection runs, subject to the same compact-panel and text-focus policy.

For physical verification, add occurrence-path picking, assembly handles,
joint-limit feedback, scoped capture/revert and simultaneous navigation to the
checklist above. Device screenshots, software-injected events and physical
Pencil/palm-rejection/VoiceOver observations must be recorded separately in
[HANDOFF.md](HANDOFF.md); this document makes no hardware-verification claim.

## Drag resolution

Length sliders and canvas parameter handles use a **0.1 mm** step by default.
The native More menu exposes **Drag resolution** with 0.01, 0.1, 0.5 and 1 mm
choices and **Continuous** (the previous unrounded behaviour). The preference is
saved as `SolidCraft.workspace.dragStepMM`; it is independent of sketch snapping.
Exact typed expressions and angle controls retain their existing precision.
Canvas drags round the absolute proposed length and retain their original contact
anchor, so small movements accumulate and reversing direction does not drift.

`view.drag_precision {"step_mm":0.1}` configures the viewport adapter. Input is a
finite number: zero selects continuous movement; otherwise 0.001–10 mm is accepted.
Its output is the viewport diagnostic object, including `drag_step_mm`. Invalid
values return `bad_params` without changing the preference or active interaction.
Changing resolution cancels current contacts. The native menu disables this choice
during numeric/modelling edits. This is a presentation preference, never design
geometry or a new engine command; desktop/web behaviour is unchanged.
