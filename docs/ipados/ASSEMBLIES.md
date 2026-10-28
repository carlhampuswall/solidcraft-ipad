# Assemblies and joints (Step 8)

## Implemented workflows and command bindings

Assembly presentation builds on Step 7's native forms and disposable
`Session::scratch` / `ComputedCommands` transactions. Definitions, placements,
joint snaps, solving, history and native persistence stay in the existing shared
engine and document. The assembly is not reconstructed from recently executed
commands or rendered mesh transforms.

| Workflow | Native API and semantics | Native iPad access |
| --- | --- | --- |
| Components | `component.create`, `component.from_bodies`, `component.move_bodies`, `component.rename`. Body transfer creates native ownership history and preserves world pose within its native placement rules. | Assembly forms, body-selection lists, destination context and browser rename. |
| Instance versus independent copy | `occurrence.copy` shares its component definition; `component.paste_new` independently copies supported definition/history. Native copy notices identify retained external links, frozen reassigned geometry and omitted boundary joints. | Separate linked-copy and independent-copy browser actions; returned notices remain visible after Apply. |
| Exact placement | Native `Sel::Occurrence {path}` and `Session::occurrence_placement`, `occurrence_children`, `occurrence_bodies`. Paths contain IDs from root to the placement. | Hierarchical browser rows and Pencil occurrence filtering use exact paths, including duplicate labels and repeated components. |
| Activation | `component.activate {component,occurrence,path}` selects the authoring placement. Geometry belongs to its shared component definition. | Browser Activate, root-context action and visible current authoring context. |
| Movement | `occurrence.move {occurrence,path,space,translate,axis,angle,origin,capture}`; `occurrence.drag {path,at,point}` moves a supported floating chain. Native numeric move angles are radians; `30 deg` is explicit. | World/local translation, rotation axis/pivot, precise unit-aware fields, Pencil translation/rotation handles and explicit Apply. Free move refuses grounded or jointed placements; constrained workflows remain separate. |
| Grounding and view state | `occurrence.ground` changes its shared occurrence definition. `browser.visibility {paths,visible,isolate}` addresses placement visibility. | Ground/unground, hide/show, isolate and show-all browser actions. |
| Position capture | `component.capture_position {occurrences?:[IDs]}` captures listed pending definitions; omission retains capture-all. `component.revert_position {occurrences?:[IDs]}` drops selected pending positions. | Pending badges, scoped Capture/Revert actions and multi-selection forms. Unrelated pending positions remain pending. |
| Joint creation | `joint.create`, `joint.as_built`, `joint.origin`; origins remain native point/frame, face, circular-end, edge/corner or named-origin references. | Rigid, revolute, slider, cylindrical, pin-slot, planar and ball; mating versus as-built creation, named origins, selected geometric inputs, component/world frames and explicit world-coordinate frames with axis choices. |
| Joint editing | `joint.edit` retains the original ID; optional `a`/`b` rebind its origins. Type, flip, offset, angle, suppression and name use the shared command. | Existing-joint edit, Keep/reference/frame input choices, explicit reselection and suppression actions. |
| Limits and motion | `joint.limits`, `joint.drive`, `joint.reach`. Values, limits, solver residuals and stop reasons come from native replies. | Typed degree/mm values, multi-DOF fields, limit index/clear controls, single-DOF Pencil handles, constrained reach and precise target entry. |
| Diagnostics | `joint.list` supplies fixed placements, DOF, floating chains, conflicts and errors; `joint.inspect` supplies stored snaps, resolved frames and lost-reference reasons. | Assembly status, joint rows, frame/axis guides, endpoint highlighting and retained completion feedback. A successful dispatch alone never establishes that geometry moved. |
| Joint deletion | `joint.delete` removes the chosen joint and connected motion links according to native rules. | Confirmation describes connected-link scope and preserved motion studies that may need repair. |

Engine-supported capabilities without dedicated Step 8 iPad controls include
`component.delete`, occurrence deletion through `selection.delete`,
`joint.rigid_group`, relation joints (parallel, perpendicular, angle,
point-on-plane, point-on-line and line-on-plane), motion links, gears,
rack-and-pinion, screws, cams, contact-set authoring and motion-study authoring.
Advanced native origin options such as separate support-face/corner/end and
x-reference selectors are not all exposed as individual controls. These native
data remain intact when loading and saving; an unexposed capability is not
reported as an unsupported engine operation. There is no fabricated joint,
independent motion solver or display-mesh substitute.

## Interaction, preview and history

Assembly uses the existing modelling inspector, reference lists, unit-aware
numeric/expression entry and `ModelEdit`/`PreparedModel` transaction boundary.
Preselected placements or geometry seed input choices; tool-first forms request
the relevant placement, target component or origin. Inputs can be replaced or
removed in the form. Selecting another joint/type/DOF rebuilds the appropriate
units, limits and fields instead of retaining another joint's values.

Pencil selects placements and manipulates the visible engine-parameter handles;
fingers keep the established navigation ownership. Navigation suspends and
rebases an active Pencil edit. Consumed controls, resize, interruption and
cancellation preserve that ownership contract. A Pencil lift leaves an assembly
proposal awaiting explicit Apply rather than silently committing motion.

Geometry is evaluated off the UI path on `Session::scratch`. Preview preparation
does not mutate the authoritative document or its undo history. The bridge
checks document identity/revision, interaction token and proposal version before
publishing or accepting work. The bounded coalescing queue retains the newest
proposal, stale completions are rejected, and editing a field disables Apply
until a current validated preview exists. A final commit revalidates against the
current document, accepts one native `ComputedCommands` transaction and produces
one undo action. Duplicate confirmation cannot create a second joint. Cancel
drops provisional work without undoing unrelated committed edits.

Native solve errors/conflicts refuse a validated preview. Clamped actual values,
no-motion reasons, limit/loop/contact stops, unreachable-target residuals and copy
notices remain available after the draft is consumed. In particular, an ungrounded
or suppressed joint's stored value can change while its placement does not move;
the actual `moved`, reason and world geometry determine the displayed outcome.

Native `file.save_as` / `doc.open` preserve component definitions, occurrences,
parametric feature history, joint IDs/origins/expressions/limits, suppression,
metadata and supported view state. Pending placement state follows the native
capture/revert contract; provisional UI proposals are not saved as CAD data.
The existing local save/reopen actions remain available. Full document-browser
and Files-app workflows are outside Step 8.

## Placement, frames and reference limits

A nested occurrence definition is shared by every instance of its parent
component. Moving, grounding or deleting that definition changes all such
placements. `Session::occurrence_scope_count` gives the affected placement count.
Use full paths and the session's effective placement APIs, which include pending
positions; do not parse displayed body/occurrence names or deduplicate repeated
placements by component ID.

The current `JointOrigin` stores an occurrence ID and native `Snap`, not a full
occurrence path. Its resolver uses that occurrence's first parent placement.
Independent joint attachment to one nested placement under repeated parents is
therefore unsupported by this revision; an adapter must reject the ambiguous
choice rather than silently use a different parent frame. Step 8 does not extend
the format to invent per-placement joints.

Joint origin creation reads captured document transforms. A pending occurrence
position or pending parent must be explicitly captured or reverted before
supplying world origin picks. Drive, edit, limits and reach also validate pending
positions in the affected native mechanism and linked chains; an unrelated branch
can remain pending. Capture scope stays explicit.
`joint.as_built` keeps the supplied parts in place, while ordinary joints mate
their origin frames. `flip:false` mates opposed z axes; two equal world axes need
`flip:true` to remain aligned. Mixing a point's inferred x axis with a separately
specified frame can turn a part, so both origins must show their actual frames.

Joint numeric values and limits use degrees/mm. Native offset, angle and initial
value expressions that reference parameters remain expressions. Driving a value
intentionally drops its prior expression and evaluates supplied text once.
Limits and placement transforms are native numeric data; the adapter must not
promise persistent expression relationships the format does not support.

The native solver has an implicit anchor rule: if none of the active joints
touches the root or a grounded occurrence, it fixes the first active joint's A
side for that solve. This does not set the occurrence's saved `grounded` flag.
Read the solver's actual `fixed`/DOF report and distinguish that implicit anchor
from an explicitly grounded part; do not label every ungrounded joint chain as
freely movable or silently ground its first part. A separate disconnected chain
can still be reported as floating.

Strict validation in `assembly_references.rs` uses the public native named-face
and named-edge lookup diagnostics, actual stored body offsets, surface kinds,
`joints::end_faces` and `joints::resolve_origin`. Missing or fallback face, edge,
corner and X-reference identities refuse a validated preview and name the joint
side that needs reselection. Explicit Point/Frame origins remain valid inputs.
Legacy `CircleCenter {end:None}` origins keep the engine's nearest-end semantics
for compatibility. The strict helper deliberately permits that native legacy
case; it does not prove persistent endpoint identity. The current form has no
dedicated end-face selector. Loaded originals are preserved, and Step 8 does not
claim to solve general persistent-topology naming.

## Rendering and placement feedback

The adapter retains native immutable source meshes and exact placement matrices;
rendering, bounds and picking use the same evaluated world placements. The bridge
uses the existing public GPU `SharedTris`/instancing API for eligible opaque
assembly faces. A bounded 256-entry cache retains source Arcs and converted
triangles across placement-only changes; monotonic keys avoid identity reuse
after eviction. A source geometry change requires new triangle data. Selection
and hover retain the exact chosen instance rather than highlighting every copy.

Existing world-space edges, sketch lines and guides keep their established
conversion. Glass, per-face colours and mesh inputs that cannot safely take the
instancing path use the existing appearance-preserving fallback. Scene
construction and these fallback paths still do work when placements change.
Deterministic tests establish sharing, bounded retention and correct appearance;
the captured frames establish actual rendering. Neither is a physical-device
performance benchmark or evidence of zero uploads or constant-time movement.

## Shared-code delta ledger

| Shared file | Why a platform adapter alone was insufficient | Minimal change and compatibility |
| --- | --- | --- |
| `crates/engine/src/cmd/joints.rs` | Existing `joint.edit` could change numeric/type/display fields but could not replace either origin. Deleting/recreating a joint loses its stable identity and can disconnect links. Direct adapter mutation would bypass the shared command boundary. | Optional `a`/`b` use the existing `origin_param` and frame/Snap validation, check that the two occurrences differ, update the original joint and run the existing solver. No new geometry, solver, joint data field or file format. Calls without these keys retain existing desktop/web behavior. Future upstream integration must retain or adopt the equivalent original-ID rebind API and its source-frame rules. Independently upstream-eligible command improvement. |
| `crates/engine/src/cmd/component.rs` | Capture Position previously committed every pending move. A zero-distance `occurrence.move` workaround still refuses fixed/rigid-group members and applies movement semantics; it cannot safely provide general scoped capture. | Optional `occurrences:[IDs]` validates the full list before mutation, captures only those existing pending transforms and leaves others untouched. Omitted scope preserves the old capture-all behavior; `[]` captures none. No new saved data. Desktop/web callers gain an additive option. Future integrations should preserve the default and scoped undo contract. Independently upstream-eligible command improvement. |

Regressions live in the existing `cmd/joints_tests.rs` and
`cmd/occurrence_tests.rs`, respectively. All other Step 8 work belongs in the
established iPad adapter and host boundaries. No assembly graph or solver is
copied into platform code.

## Executed validation

The following results were read from the completed Step 8 logs. No simulator was
booted or launched for these checks.

| Check | Recorded result and evidence |
| --- | --- |
| Safe session adapter and bridge tests, Rust 1.95.0 / `ci` profile | **PASS: 105 session tests and 23 bridge tests**, including earlier sketch/input/modelling regressions; `/tmp/solidcraft-step8-adapter.log`. |
| Existing shared engine joint tests | **PASS: 163 tests**, including original-ID origin replacement, expressions, malformed input and native round-trip; `/tmp/solidcraft-step8-engine-joints.log`. |
| Existing shared occurrence tests | **PASS: 77 tests**, including scoped capture, unrelated pending positions, empty/invalid scopes, capture-all compatibility and undo/redo; `/tmp/solidcraft-step8-engine-occurrences.log`. |
| Focused iPad lint | **PASS**, `/tmp/solidcraft-step8-clippy.log`; this is not the full workspace lint gate. |
| Native presentation checks | **PASS: 186 adaptive layout/focus/command-decoding cases**, `sh apps/solidcraft-ipados/scripts/check-workspace.sh`; `/tmp/solidcraft-step8-native-check.log`. |
| Unsigned native device-target build | **PASS**, `sh apps/solidcraft-ipados/scripts/build.sh`; `/tmp/solidcraft-step8-native-build.log` ends `BUILD SUCCEEDED`. Compilation/linking does not establish device runtime. |
| Shared desktop/web compilation compatibility | **PASS: desktop app all-targets check**, `cargo +1.95.0 check --locked --offline --profile ci -p solidcraft --all-targets`, `/tmp/solidcraft-step8-desktop-check.log`; **PASS: 14 crates for wasm32**, `cargo +1.95.0 xtask wasm`, `/tmp/solidcraft-step8-wasm.log`; **PASS** `cargo +1.95.0 xtask web-offline-test`, `/tmp/solidcraft-step8-web-offline.log`. Existing wasm warnings are recorded. No desktop UI session was tested. |
| Dependency boundary | **PASS: 13 shared crates / 177 resolved CAD packages**, `/tmp/solidcraft-step8-dependencies.log`; no new kernel, solver or platform dependency. |
| Mandatory full repository gate | **BLOCKED**, `cargo +1.95.0 xtask ci`, `/tmp/solidcraft-step8-ci.log`: format, assets, icon checks, licences, lists, skills, layers and container checks pass; workspace Clippy stops on inherited `clippy::nonminimal_bool` at `crates/geom/src/fill_general.rs:166`. Later full-gate stages did not run. |

The session acceptance cases exercise component transfer, shared instances versus
independent copies, upstream geometry regeneration, exact nested paths and
duplicate labels, world/local transforms, activation, visibility/isolation,
grounding, floating rigid-chain drag, scoped position capture and native
round-trips. Joint cases cover a grounded hinge, slider, as-built rigid joint and
the other exposed basic types, original-ID editing, expressions, typed/mixed-unit
limits, actual clamped values, suppression, no-motion reasons and undo/redo.

Deterministic controller tests cover invalid/lost face/edge/end/x references and
explicit same-joint repair; repeated nested origin refusal; pending parents and
connected mechanisms; Pencil translation/rotation/drive/reach; finger navigation
and UI ownership; interruption/cancel; reordered preview completion; document
changes; stale publication and duplicate confirmation. These tests use actual
engine geometry and controller paths, not arbitrary sleeps or screen-only checks.

### Real macOS GPU and shared CLI evidence

`cargo +1.95.0 build --profile ci -p solidcraft-ipados-bridge --example render_proof`
builds the existing proof executable. The added invocation
`target/ci/examples/render_proof /tmp/solidcraft-step8-assembly.png --assemblies`
ran on the Apple M3 Pro Metal adapter after the sandboxed attempt returned
`NoAdapter`. It creates four native parametric boxes in Grounded Base, Arm, Pin
and Carriage components, with one revolute, one as-built rigid and one slider
joint. Their native total is 5056 mm³, four world bodies and three joints.

The actual drive reply clamps a 120° hinge request to 90°, then accepts 30°; the
slider clamps 75 mm to 50 mm, then accepts 20 mm. Native occurrence physical
properties check Arm/Pin/Carriage world centroids at each committed boundary.
At the final pose Arm is `(43.3012701892,25,0)` mm, the rigidly attached Pin is
`(86.6025403784,50,0)` mm and Carriage is `(20,20,0)` mm, within 1e-6 mm.
Grounded Base remains at the origin. Preview preparation leaves captured
placements unchanged; cancelling a provisional −15° hinge solve restores the
committed 30°/20 mm pose. Saving and reopening preserves the same geometry and
joint relationships.

All eleven generated images were inspected: `before0`, `hinge90-preview`,
`hinge90`, `hinge30-preview`, `hinge30`, `slider50-preview`, `slider50`,
`slider20-preview`, `slider20`, `cancel-preview` and `cancelled`, under
`/tmp/solidcraft-step8-assembly-*.png`. They are actual shared GPU model/feedback
captures, not UIKit screenshots or physical-iPad evidence.
`/tmp/solidcraft-step8-gpu.log` records the replies/poses and final PASS;
`/tmp/solidcraft-step8-assembly-native.json` records the native diagnostics.

The same `/tmp/solidcraft-step8-assembly.solidcraft` document was read by a current
`cargo +1.95.0 build --profile ci -p solidcraft-cli` build, using CLI `eval` and
an inspection script with `run --in ... --results`. Comparison verifies component
definitions and occurrence transforms, three joint IDs/frames/limits, four
parametric feature identities, parameters/metadata and occurrence world physical
properties, with 1e-7 mm absolute numeric tolerance. Transient session
revision/undo/redo/dirty state and measured evaluation milliseconds are excluded.
Logs and results are `/tmp/solidcraft-step8-cli-build.log`,
`/tmp/solidcraft-step8-cli-eval.json`,
`/tmp/solidcraft-step8-cli-inspect-results.json` and
`/tmp/solidcraft-step8-cli-compare.log`. This establishes shared CLI compatibility;
it does not establish desktop UI testing.

### Physical runtime status

The signed app runs on the physical iPad Pro11-inch(3rd generation), iPadOS27.2,
using the existing development identity. The opt-in DEBUG `--assembly-check`
invokes the production UIKit form Preview/Apply/Cancel callbacks and waits for
matching presented document/revision/preview frames. Its paused variant waits for
SIGUSR1 at six READY stages; no arbitrary sleep advances a CAD operation.

**PASS:** actual revolute/slider creation, hinge preview90/cancel0, commits90/30,
Undo90/Redo30, requested slider75→native50mm, unique native save, mutation20mm,
reopen50mm and stable joint IDs11/12. These are automated controller actions on
real UIKit/Metal hardware, not human Pencil events. The copied physical native
file also passes shared CLI evaluation: four closed bodies,6280mm³, original
feature IDs3/6/9/10, no feature errors/warnings; hinge30/slider50 centroids match
independent analytic poses within1e-7mm. The broader rigid/as-built and additional
joint workflows retain the deterministic/Mac GPU coverage recorded above.

Logs are `/tmp/solidcraft-step8-{signed-build,device-check,device-native}.log`;
the copied native file is `/tmp/solidcraft-step8-ipad-runtime.solidcraft` with
`/tmp/solidcraft-step8-ipad-cli-{eval.json,compare.log}`. All six stages were
captured/inspected in the physical Device Hub live view. Raw CoreDevice PNGs have
an orientation/cropping defect on this beta SDK/device combination; the user
confirmed full-screen app layout, and native telemetry confirms matching scene,
window, view and canvas dimensions. Those malformed PNGs must not be presented as
complete UI evidence. Device screen recording is unsupported. Device inspection
also prompted readable rail/panel controls and canonical command-label lookup;
186 adaptive cases plus label-mapping/fallback checks pass.

The user verified physical occurrence selection and finger navigation, and
reported rotation-preview flicker. A held-worker regression reproduced the
per-sample committed-pose fallback. The adapter now keeps one display-only
last-good preview while the next value is unvalidated, labels it stale and
disables Apply; current results replace it and cancellation drops provisional
work. Numeric/slider evaluation uses the same retained-display boundary; source
selection explicitly reveals the authoritative model. The new deterministic
regression passes with the full 105-test session
suite. The fixed build was then installed and the user repeated the physical Pencil
ring drag and Properties/Cancel, reporting a stable preview. Selection, basic
finger navigation and this constrained preview/cancellation workflow therefore
have human device verification. Palm rejection, simultaneous finger/Pencil
arbitration, VoiceOver and device performance remain separate outstanding
checks in [HANDOFF.md](HANDOFF.md). No device performance result is claimed.
