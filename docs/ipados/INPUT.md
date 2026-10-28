# Native Pencil and multitouch input (Steps 4–8)

The development host retains one source-aware pipeline. The original Step 4 tool
remains available alongside the later sketch, modelling and assembly tools.
Select **Post radius**, then drag the cross at the cylindrical post's top rim.
The handle edits that fixture feature's `radius` with `timeline.edit` (0.5–50 mm).
Select remains the initial tool. Undo/Redo and Command-Z/Shift-Command-Z use engine
history; Escape or Cancel interaction discards a provisional interaction.

## Gesture mapping

| Input in the viewport | Action |
| --- | --- |
| Pencil tap in Select | Visible enclosed sketch region or actual occurrence/body/face/edge/vertex/Auto picking; Multiple still applies |
| Pencil drag on the Post radius handle | Actual plane intersection → provisional solid → one command on completion |
| Pencil drag on an active modelling/assembly handle | Native parameter proposal and scratch geometry; lift leaves a preview for explicit Apply |
| One finger | Turn the visible object in the drag direction in 3D; pan in an active sketch plane; an enabled direct handle captures its own drag |
| Two fingers | Simultaneous centroid pan and distance-ratio zoom |
| Two-finger double-tap | Undo one engine history action |
| Three-finger double-tap | Redo one engine history action |
| Pointer primary / hover | Existing selection / preselection |
| Secondary drag / middle or Shift-secondary | Existing orbit / pan; planar mode maps orbit to pan |
| Wheel or trackpad scroll | Shared zoom path |
| Input beginning on a native control | That control owns it until UIKit completes/cancels it |

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
falls behind. It retains begin/end/cancel, reversals and context boundaries;
only intermediate moves on the same screen path (within a quarter point) can be
removed. If no redundant motion remains, overload still cancels conservatively.
The Foundation-only input values and mailbox live in `WorkspaceLayout.swift` so
the native harness exercises production batching without UIKit or a simulator.
Stationary handle samples do not supersede an in-flight geometry preview. Tapping
a handle releases capture, and a new grab supersedes the previous lifted contact's
pending preview. Preview completion still checks the current interaction ticket.

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

Pencil movement beyond 5 display points distinguishes drag from tap. Finger
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
