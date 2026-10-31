# Native floating CAD workspace

The UI workstream starts at source baseline
`c22acd12702cf3a405df01009b8d8662b3d46365` (B), using finalized launch policy
`380b43b2047a1242c81dfec75142283a0b61bd76` (P). The provisional blocked policy
inside B is superseded by P. The original `codex/ipados-ui` implementation and
later native work are consolidated on `develop`; [DEVELOP.md](DEVELOP.md) records
the source history and current verification scope.

## Current presentation

`WorkspaceController.swift` hosts one real `MetalCanvas` behind native floating
controls. The document header is a centred surface, normally 340 × 52 points,
showing the actual document name and app-private persistence status. Its folder
menu opens Projects, Save project, Import, Export and Share STEP. The separate top-left
Model button opens the model browser. Projects presents the native design library
with search, create/open, and context-menu duplicate/delete actions. Switching
saves first; current projects cannot be deleted. Imports can open as a separate
project or insert geometry into the current one. Export offers STEP, SolidCraft,
STL, OBJ and 3MF through Files. Share STEP exports the current committed design
and opens the system share sheet with its named `.step` file for AirDrop and
compatible apps. Temporary exports remain available until the activity finishes
or is cancelled. See INTERFACES.md for ownership and limits.

On iPadOS 26 and later, the floating document header moves up into the centre of
the top status strip when it fits, retaining its full height and menu touch area.
It leaves 240 points at each end for system indicators and window controls.
Narrow windows, a browser that shifts the header too close to an edge,
accessibility-sized headers, and older iPadOS versions retain the safe-area
placement. The rest of the chrome and the Metal viewport do not move.
Changes a default: yes — **Tools → Use top strip for document header** is enabled;
turning it off restores the previous header position
(`SolidCraft.workspace.useTopSafeArea`).

The floating header hosts its folder button directly, with a full 44-point
touch target. It does not wrap that single control in a scroll view: UIKit's
scroll-edge effect could blur the icon when the header occupied the top strip.
The compact floating workspace is the only available layout. The opt-in `--chrome-check`
also checks native hit testing across the visible controls and captures the
floating header in Light and Dark appearances.
Physical iPad verification on 2026-10-29 passed all eight text/floating,
Light/Dark and bottom-spacing variants, including nine native hit-test points
per visible enabled control. Captures show the folder icon without the blur,
and a real pointer click opens its Projects/Save/Import/Export menu. The unsigned
and signed device builds and 550-case Foundation layout harness passed.

The floating viewport fills the entire app window, including the space behind
the status and home indicators. Apart from the document header described above,
controls retain the top and side safe-area insets and use the bottom safe area by
default, leaving only their normal edge spacing.
There are no reserved black bands above or below the model. The workspace itself
also fills the window, so the extended viewport remains a real native input
surface. Pinned browsers retain their horizontal reservation. The keyboard still
reduces the usable viewport height through the existing resize/cancellation path.

The 52-point tool rail (56 for accessibility text) contains icon-first controls
with at least 44-point interaction regions. Modelling groups expose the actual
native-supported create, modify, pattern/mirror and assembly forms. Additional
native-supported operations remain reachable in the create menu. Sketch editing
replaces these with Line, Rectangle, Circle, Arc, Spline, Trim, overflow tools and
Finish chain. A labelled **Finish sketch** button occupies its own row in the
document header, outside the scrolling rail. Cancelling a segment retains its
icon-only presentation after state refreshes and plane-picker cancellation.
Constraints, dimensions and construction appear
in the opposite rail only while sketching. Constraint menu eligibility and
dimension types come from `eligible_constraints` / `eligible_dimensions`; the UI
does not decide geometry compatibility or construct new constraint semantics.

View orientation, Fit, projection and Properties use the existing viewport
commands. Active sketches omit unavailable 3D orientation/projection actions.
**Views → Visibility → Axes in grid** is enabled by default. The current XY,
XZ or YZ grid carries its two world-coloured axes in both directions, fading
with the grid; the perpendicular axis stays hidden automatically. Sketches use
their own plane and origin, including offset and tilted local grids. The corner
orientation triad continues to show XYZ independently. Automatic grid axes have
no reference labels or hit targets, preserving empty-space selection clearing.
Individual axis visibility overrides still win: explicit Show exposes the
selectable engine reference and can show an off-plane axis; Hide suppresses an
axis even on the current grid. Only axes along the visible grid extend in both
directions; off-plane axes retain short positive origin markers. Automatic
restores the plane-dependent defaults. Hiding the grid hides its automatic axes;
explicitly shown references remain as short markers, including axes that were
previously extended along that grid.
Changes a default: yes — turning off **Axes in grid** restores the former hidden
origin-axis defaults and short positive reference segments
(`SolidCraft.workspace.integratedGridAxes`). The preference and individual
overrides persist between launches without changing document history.
There is no second orientation cube or camera path; the existing native triad
still describes the real camera. Undo/Redo occupy a small bottom surface and
retain their existing history availability checks and keyboard/gesture ownership.
Status is a fitted two-line pill reporting actual selection, solver/tool/snapping,
processing or errors. Routine assembly diagnostics and the Post-radius development
tool are available through **Tools → Show diagnostics**.

Browser and properties panels start closed and float inside their own visible
bounds. Opening an unpinned panel leaves the Metal view's origin and dimensions
unchanged; UIKit consumes only contacts inside controls/panels. No scrim captures
the remaining viewport. The optional **Pin model browser on wide windows**
preference reserves browser space at widths >=1000 points (1180 with accessibility
text); closing it restores the viewport. Floating mode leaves 100% of available
content width assigned to the viewport when the browser is unpinned. Controls
overlay its edges. This is a layout measurement, not a claim that every pixel is
uncovered. Short numeric transactions use a 320 × 500-point floating editor,
bounded by available height; larger forms and keyboard-reduced windows scroll.
Preview validation, Apply/Cancel and original feature/occurrence identity remain
owned by the existing session and forms. Closing Properties or tapping empty
viewport space now cancels a modelling/assembly draft and restores the normal
tool rail. Camera drags and taps on geometry keep the draft active. **Tools →
Cancel tools when dismissed** can be disabled to restore the earlier behaviour
of hiding the form while retaining its draft. The form's explicit Select action
still hides the panel to collect references.

`WorkspaceLayout.chromeBounds` retains top/side insets and extends the available
control height to the window bottom, bounded by the keyboard.
`WorkspaceLayout.resolve` uses these display points, window size, Dynamic
Type, keyboard intersection and measured stack content heights. `placed(in:viewport:)`
translates chrome into window coordinates and extends the floating canvas to the
window edges. It shrinks or scrolls
chrome instead of introducing fixed screenshot dimensions. Actual canvas changes
(pinning, keyboard, rotation) still run the original cancellation/drawable-metrics
path. The Foundation harness retains the previous layout tests and additionally
checks floating panel invariance, width targets, pinning thresholds, transient
dimensions and short editor/navigation surfaces.

Native labels, values, selected/disabled traits, pointer tooltips and the large
content viewer retain meaning when labels are visually collapsed. Menus and
forms show text. Icon selection does not depend on colour alone. Physical
VoiceOver and simultaneous Pencil/finger coverage still require human checks.

## Compact operation inspector

Modelling and assembly forms use the panel header for their operation or edited
feature title. The scrolling body retains the Rust-authored descriptor fields
and raw expressions. A reference summary offers **Change**, including the
existing viewport selection action. Unknown references show a selected-input
label rather than command JSON. Numeric fields have one label; Extrude's distance
is emphasised. **Expression help** discloses the existing syntax instructions.

Extrude presents source, distance and operation first. Its two-by-two operation
choices preserve the engine's option values and order: New Body, Union,
Intersect, Subtract. They use the existing `body`, `combine`, `intersect` and
`cut` masters through `WorkspaceIcon`, with text, a selected surface and a
checkmark. The canonical Cut master is a scissors cue, so the visible Subtract
label remains essential; no generated cube/subtraction artwork is shipped.
Direction, through-all, targets and all other descriptor fields remain available
under **More options**. Reference order and the automatic-subtract/explicit-override
rules still belong to Rust.

Automatic preview status and Cancel/Apply occupy a persistent panel footer outside
the scroll view. The actions stack vertically when their labels cannot fit
across, including larger text and narrow windows. Detailed engine feedback
scrolls with the body. Apply validates the current parameters and commits directly;
Edits preview automatically after a short typing pause. Only the latest queued
draft is evaluated; Apply and Cancel discard delayed work. The async generation/document/token checks,
preview mailbox and atomic engine acceptance are unchanged. Compact forms request
380 points of width, bounded by available space. The floating Metal canvas retains
its bounds when the inspector opens; the existing keyboard resize path remains.

Changes a default: yes — **Tools → Compact operation inspector** is enabled
(`SolidCraft.workspace.compactInspector`). Turning it off before starting a form
restores the previous repeated headings, field order and scrolling actions. The
setting is unavailable while a modelling draft is active.

`--compact-inspector-check` is an opt-in DEBUG physical-device diagnostic. It
uses a disposable in-memory model and verifies primary-field visibility,
persistent action targets, matching-preview eligibility, raw-text/reference
invalidation, keyboard bounds, canvas invariance and the previous preference.
It writes `Documents/compact-inspector-{dark,light,keyboard,legacy,subtract}.png` using the
actual UIKit hierarchy. The app-window captures exclude separate system keyboard
windows. This does not establish hardware Pencil, VoiceOver or palm-rejection
coverage. No library project or saved preference is changed by the diagnostic.

2026-10-29 verification: the unsigned and existing-profile signed builds, native
layout harness, locale/icon/asset/container checks, 18 extrusion tests and five
subtract tests pass. The physical diagnostic passes primary-field visibility,
canvas invariance, Apply invalidation, real Subtract-button preview, Light/Dark
refresh and legacy layout. Actual field focus and the observed keyboard/accessory
boundary (782 points on the 834-point landscape window) retain the footer above
that boundary. Synthetic UIKit inspector layouts at widths 320/380 and
accessibility XXXL retain all action targets. Corrected native captures were
inspected. Local evidence is in `target/task1-inspector/`; the normal saved Test
document was restored afterwards. Full software-keyboard visual coverage and
hardware Pencil/VoiceOver remain separate follow-ups.

## Numeric expression entry

Numeric entry follows the approved keypad reference: the popup contains its own
four-by-four number pad, a large outlined raw-expression field, field name and
native source context, a compact units/parentheses/ABC row, framed expression
help and one persistent Cancel/Apply row. The pad's Apply key submits the
current expression directly, using the same action as the footer. The number pad
is a UIKit view inside the popup; numeric mode does not open a separate
software-keyboard area or resize the Metal canvas.
ABC explicitly opens full text entry for parameters, functions and additional
operators; its 123 accessory returns to the embedded pad. The same text field
retains caret, selection and native text undo through both modes. The sign key
wraps the complete raw expression in a unary-minus expression and can restore
that same draft with a second tap; only Rust evaluates it.

Tapping a compact inspector expression field opens this same popup, as do the
existing on-model values and sketch dimensions. The stable workspace retains the
original source/annotation rectangle when underlying form fields are rebuilt.
The legacy inspector preference retains direct full-keyboard text entry. The
popup keeps its actions outside the scrolling body and adapts to narrow windows,
keyboard entry and Dynamic Type. Keys have at least 44-point targets. Optional
native operation choices remain under its help disclosure.

The saved System/Light/Dark choice reaches the presentation environment and
input views. Semantic colours and scoped keypad styles leave menus, browser and
rail appearance alone. New Extrude and Offset Face distance descriptors publish
the engine Length kind's default unit, so Swift does not infer units from labels
or parse expressions. Other fields retain their native unit metadata.

Automatic preview leaves text editable and validates only its captured draft
revision. Number edits, modelling parameters/references and sketch transforms
preview after a 300 ms typing pause; existing sliders and direct handles retain
their live preview path. One pending request replaces older queued edits.
Dimension and modelling Apply validate current values before accepting one
transaction. There is no manual Preview button or preview-required setting;
the former preference key is ignored, as explicitly requested by the user.
Errors appear beside the retained draft. In numeric popups, keypad Apply, the
full-keyboard accessory's Apply and Return all submit the current expression in
one step. Inline fields without transaction actions keep Return as input dismissal.
Applying an operation disables Close/Cancel and edits until completion.
The existing Rust tokens, context checks, scratch evaluation and atomic
acceptance still own geometry and history.

2026-10-29 verification: native layout/focus/decoder checks (678 adaptive cases),
canonical icons, pending locale lists, UI containers and unsigned/existing-profile
device builds pass. All 169 session and 28 bridge tests pass, as do affected
Clippy with warnings denied and workspace formatting. The existing face-extrusion
regression also checks the native distance-unit metadata. DEBUG
`--numeric-entry-check` uses a disposable fixture and covers actual in-panel
keys, sign edits, units, native Preview/Cancel, invalid expressions, rapid edits
and stale completion, retained focus/selection, ABC/123, viewport invariance and
opening the original Extrude Distance field from its inspector. It also checks offscreen UIKit layouts at 320x300,
380x500 and 744x1000 with normal and accessibility XXXL text. Those sizes are
synthetic, not physical rotation or multitasking observations. The 2026-10-31
one-step confirmation extension also checks invalid and valid submissions from
the keypad, Return and full-keyboard accessory, with one Undo per acceptance.

Actual native captures include the complete embedded pad and were inspected in
light/dark and invalid/stale states. Final reports, captures and logs are local
ignored files in `target/task-2-embedded-pad/`. The earlier
`target/task-2-numeric-entry/` captures describe the superseded keyboard-area
implementation. Hands-on Pencil, VoiceOver and hardware-keyboard input remain
unverified. No simulator was used, and this scoped change does not claim a full
workspace CI run. Diagnostic fixtures never load or autosave library designs.

## Canonical icon bridge

`scripts/generate-icons.py` reads the existing SVG masters, optical variants,
`aliases.toml` and `palette.toml`. It calls the existing `icon_palette.py` resolver
for semantic accents, guide paints and transparent knockouts. It never modifies
masters, palette, aliases or review hashes. No artwork is redrawn or marked final.
It generates light/dark SVG vector image sets in Xcode's target temporary directory;
the asset compiler packages them in `Assets.car`. No generated catalog or copied
SVG is committed. Original masters and their existing attribution remain the
authority. Python 3.11+ is build tooling only, already a repository prerequisite.

The same pipeline extracts `.icon(...)` from upstream `CommandSpec::new`
declarations into `SolidCraftCommandIcons.json`. This is presentation metadata,
not a second command registry. `WorkspaceIcon.command` uses this generated map;
future native-supported operations inherit the canonical icon automatically.
Chrome uses existing semantic icon IDs. UIKit resolves vector density and theme;
`WorkspaceIcon` caches raster images by ID, appearance, display scale and size,
bounded to 256 entries. Selected controls retain semantic artwork; disabled
controls use native availability. Missing artwork keeps a readable label.

The user explicitly authorized the resource-only Xcode project registration
outside C's original ownership. No build/signing setting, app lifecycle,
`NativeSession`, `MetalCanvas`, Rust production or ABI contract changed.
`check-workspace.sh` verifies 41 actual native tool mappings, both appearances,
vector preservation, retired-output removal and unchanged canonical resources.
At implementation time the pipeline consumed 321 masters, 37 aliases and 481
registry icon mappings; these counts are discovered rather than fixed limits.

## Defaults and compatibility

Compact floating chrome is always enabled. The **Compact floating workspace**
toggle has been removed at the user's request; saved
`SolidCraft.workspace.compactChrome` values are ignored, including `false`.
The earlier text rail, header and docking policy are no longer available.
Dark native chrome remains the default.
**Tools → Appearance → System** restores the earlier appearance policy; an
existing `SolidCraft.workspace.appearance` value is retained. Rail side and
two-/three-finger history gesture preferences are unchanged. Pinning and diagnostic
visibility are transient native presentation preferences, never CAD data.

Changes a default: yes — bottom controls now use the bottom safe area.
**Tools → Extend UI to bottom edge** off restores the former safe-area spacing
(`SolidCraft.workspace.useBottomSafeArea`). The preference is saved between
launches and applies to collapsed/expanded history,
panels and on-canvas model value placement. Keyboard bounds still take priority.

## Viewport appearance and visibility

The supplied assembly screenshot was produced by the opt-in DEBUG
`--assembly-check` fixture and its explicit `view.top` command. It is the real
shared shaded/depth-tested renderer. It is not a temporary drawing or a second
2D CAD engine. The normal fixture is shown in an actual shaded 3D view on the
physical iPad with body highlighting and native picking.

The native host passes resolved semantic surface and separator colours through
`view.background`. Dark mode uses dark gray grid lines, with slightly stronger
major lines. Light mode restores the shared original palette. Only guide buffers
refresh when their appearance changes; CAD meshes retain their existing uploads.

**Views > Visibility** groups Grid lines, Origin point, X/Y/Z axes and XY/XZ/YZ
planes. **All axes** and **All planes** toggle each group independently, leaving
the origin point and the other group unchanged. A partially visible group has a
mixed checkmark; tapping it shows every member. The grid toggle applies in both
3D and sketch views, defaults to on and
persists between launches. Reference controls retain their existing individual,
show-all, hide-all and automatic-during-sketch-creation settings. They have moved
out of the Model panel; **Origin controls in Model** restores the previous browser
rows. Both menu and optional rows use the same saved reference visibility.

Theme and grid changes do not dirty the document, change geometry revision,
retessellate CAD, move the camera or accept/cancel a draft. Physical iPad checks
should verify menu states, persisted visibility and System/Light/Dark switching.

## Verification and integration

The actual unsigned device build and a signed physical-iPad build pass. The signed
build reuses the pre-existing development certificate/profile without provisioning
updates. Real Device Hub screenshots and click/keyboard checks exercise browser
open/close, native body selection, radius Preview/Cancel and sketch entry/tool
switching. These are physical UIKit observations, not mockups; hardware Pencil,
palm rejection and VoiceOver are separate outstanding coverage. See the UI section
of HANDOFF for current evidence and final gate status.

Floating chrome also has an opt-in DEBUG UIKit regression:
launch with `--chrome-check` to cycle light/dark and bottom safe-area settings,
then read `Documents/workspace-chrome-check.json` from the app container. It checks
actual visible icon/touch frames after UIKit layout, including the reparented
browser button and retained scroll offsets. It also checks that the actual
canvas fills the top/bottom window edges, that edge hit tests reach the canvas,
and that status/history use the requested bottom spacing. It does not merely
repeat the Foundation policy calculation. Earlier verification passed all eight
combinations of text/floating, light/dark and both bottom spacing preferences for
the bottom extension. The report is `target/ipados-bottom-edge-chrome-landscape.json`;
the unsigned and signed build logs are `target/ipados-bottom-edge-{unsigned,device}.log`.
On this iPad the extension recovers 25 points beneath the former safe-area bound.
The optional
`--chrome-check-restore-dark` restores the dark appearance after this check;
without that flag it restores the preferences present at launch. This mode never
authors CAD or synthesizes Pencil input. It caught a runtime reparenting issue;
the fix resets scroll offsets once after new chrome frames are assigned and
restores manual frame ownership to the floating browser button.

Review C independently, then integrate its presentation changes with B's render
contract. A's document menu/lifecycle wiring remains integrator-owned; replace the
existing local-document menu through that reviewed service without adding a
second document store. Preserve the original full occurrence paths and preview
contracts below. Rerun the native harness, unsigned build and relevant scoped
regressions on the final combined tree. Per the user's 2026-10-28 instruction,
run the engine suite only on an explicit request for a full test. The automatic
full-CI attempt for this UI change was stopped at that instruction; it is not a
completed gate. See HANDOFF for the completed checks and interrupted-run record.

---

## Established Steps 5–8 contracts (historical layout description)

The UIKit host is a thin presentation layer over the real Rust session, viewport
and Step 4 input router. It adds no CAD document, evaluator, modelling registry,
camera controller, undo stack, dependency or artwork.

## Layout and reusable components

`WorkspaceController.swift` replaces the development toolbar. The top bar shows
the actual document name, real history availability and Model/Properties/View
access. The two-line subtitle identifies the active authoring component and exact
occurrence path, alongside the local-document status. The engine's `is_dirty`
projection distinguishes unsaved edits from context changes. Panels start closed.
Feature history and assembly placements share the Model browser. Projects use
the existing native container in private storage, autosave committed edits and
restore the last project. The Tools menu preference “Reopen last project on
launch” can retain the former starter-model launch behaviour.

`WorkspaceLayout.swift` uses safe-area points and available window size. Below
1000 points wide (1120 with accessibility text), one secondary panel uses a
consuming modal scrim. Wider panels dock beside the actual Metal canvas. Both use
the same components. Header/status heights account for scaled font metrics.
Keyboard intersection reduces the workspace; the inspector scrolls to the field.
Top actions and the tool rail scroll rather than shrinking touch targets. The
text rail uses 112pt normally and 160pt for accessibility text, bounded by the
available window. Docked panels share any width deficit while retaining the
360pt canvas budget. This avoids fragmenting the assembly/sketch tool labels.
Panel Close retains a short visible title and full accessibility label; headings
can truncate before compressing its touch target. Command titles resolve the
existing canonical command or legacy UI catalogue keys rather than exposing
registry identifiers. Unknown/user text remains unchanged.
The shared camera path reversibly corrects framing when aspect changes, without
changing geometry, history or tessellation.

`WorkspaceComponents.swift` provides labelled tool buttons/groups, panel chrome,
property rows, numeric fields, Apply/Cancel and inline error/progress messages.
Browser rows use reusable native table cells. Semantic background, label,
separator, fill, accent and error tokens support system/light/dark appearance.
Primary targets are at least 44 points; rail tools are 60 high. Fonts follow
Dynamic Type and numeric results use monospaced digits. No action needs hover.

Native presentation preferences use UserDefaults keys
`SolidCraft.workspace.railRight` and `SolidCraft.workspace.appearance` (0 system,
1 light, 2 dark). They are not stored in a design or copied into egui preferences.

## Presentation mapping and browser

**Views > Visibility** contains the origin point, X/Y/Z axes and XY/XZ/YZ planes,
alongside Grid lines. **Origin controls in Model** optionally restores an
expandable **Origin references** group in the browser; each row and its eye button
toggle the same saved visibility without selecting or changing CAD geometry.
The Visibility menu offers Show all, Hide all and reset
to **Automatic during sketch creation**. With no saved override, references are
hidden during ordinary modelling and temporarily appear in the sketch-plane
chooser. Cancel/creation restores normal visibility; active sketch axes remain
part of the sketch guides. Explicit per-item choices survive app relaunch and
take precedence over automatic visibility. These rows are native presentation
controls, separate from the authoritative document rows and their parent IDs.

The origin point uses the renderer's solid dot, with no letter O or button fill
covering it. Its transparent 44-point native touch target retains the **Origin
point** VoiceOver label and selects the existing native vertex at world zero.


`session/src/workspace.rs` caches rows by document ID and geometry revision.
Components use actual parent IDs, features use owning components, and bodies use
`world_state` ownership. Sketch features are identified explicitly. No relationship
is inferred from display names. Body/feature rows carry real engine `Sel` values;
occurrence rows carry exact `Sel::Occurrence {path}` values. Nested repeated
placements have distinct full-path row IDs. Joint rows retain only a transient
inspector row ID because the engine has no joint-selection variant; their explicit
Highlight connected parts action selects real endpoint occurrences. No component
or joint selection type is invented. Face/edge/vertex picks mark the containing
body. Browser and Pencil share bounded selection and multi-selection. Ordinary
updates retain row identities; document replacement clears expansion state.
Empty/loading/error/retry states are present.

Pencil selection defaults to Auto so body edges and vertices can be picked
directly alongside faces. **Tools → Filter** is a saved presentation preference
(`SolidCraft.workspace.selectionFilter`); choosing Face restores the previous
face-only behaviour. Invalid or missing saved filters fall back to Auto. This
preference changes neither model geometry nor document history.

**Multiple** starts enabled and is saved in
`SolidCraft.workspace.multipleSelection`. The Tools menu and Properties control
share that preference: disable it to restore single-selection replacement.
Successive canvas picks accumulate; tapping a selected item removes it, while an
ordinary empty-space tap or Clear clears the set. The full edge/face set reaches
the existing modelling forms and engine commands.

| Visible action | Integration |
| --- | --- |
| Select | `tool.set {tool:"select"}` and existing Pencil picking |
| Radius | `tool.set {tool:"post_radius"}`, Step 4 handle; palette availability requires a supported Post selection |
| Edit radius / Preview / Apply / Cancel | Engine-lane `numeric` requests, upstream scratch/transactions and `timeline.edit` |
| Browser row | Viewport `select.entity`, real `Sel`, additive toggle |
| Undo / Redo | `edit.undo` / `edit.redo`, authoritative engine history |
| View menu | Existing Fit, standard views, perspective, Exit sketch view |
| Tools menu | Existing filter/multiple/clear selection and native presentation preferences |
| Model tools / feature inspector | Engine-authored `kind:model` forms, real timeline inspection/editing and dependency actions |
| Assemble / placement and joint inspectors | Engine-authored assembly forms/actions, exact occurrence paths and native joint diagnostics |

Unimplemented workflows are absent. Disabled Radius explains its context in
Properties and accessibility hints. The direct Step 4 handle remains available.
An engine command's existence does not automatically expose a native workflow.

## Inspector and numeric contract

The inspector shows real selection identities, feature inputs and assembly state.
The original fixture radius remains a supported specialised property.
`NumericExpressionEditor` keeps incomplete text locally. For the fixture radius,
Preview/Apply sends it unchanged to `Document::eval(Kind::Length)` and
`timeline.edit`. Other fields follow their native command's length/angle and
parameter contracts. Upstream owns
units, parameters, functions and validation. Decimal commas work outside
parentheses; within functions commas separate arguments and decimal points
disambiguate values. Help states this. Evaluated values use the user's output
locale without normalising input. Invalid text never becomes a fallback number.
The fixture's existing supported range remains 0.5–50 mm.

Begin captures token, document/geometry context, selection and original handle.
Preview evaluates real scratch geometry without authoritative history changes.
Apply rechecks the token after computation and accepts exactly one prepared
upstream transaction. Cancel discards scratch and reveals the original scene.
The specialised numeric transaction locks selection/tool/history controls.
Keyboard/layout changes keep the numeric token; suspension invalidates it.
Stale callbacks cannot reopen an
editor or target another document. Confirmed Apply disables Cancel while accepting.
Text undo remains native; document shortcuts are withheld during text/draft focus.

## Modelling and assembly forms

The Model and Assemble menus use Rust-authored tool lists and field descriptors;
Swift retains opaque references and raw expression text. Each form has explicit
Preview, Apply and Cancel. Geometry is evaluated on upstream scratch sessions;
only a current validated proposal enables Apply, which accepts one native history
action. Reference selection and finger navigation remain available when the
viewport is exposed. Compact consuming panels still own their covered area.

The feature inspector shows original type/order/ID, suppression, warnings,
failures and stale geometry. Edit targets the original feature. Rename, suppression,
dependency inspection and confirmed native deletion use real timeline commands.
See [MODELLING.md](MODELLING.md) for supported tools and reference repair.

Assembly rows show active/grounded/hidden/pending state and native diagnostics.
Their actions activate the exact placement, return to root, move/rotate, create a
linked instance or independent copy, change grounding, show/hide/isolate, and
capture or revert pending positions. The Assemble forms also create components
from bodies and move bodies into a chosen component placement. These operations
retain the native distinction between a definition and its shared occurrences.
Scope warnings come from the engine adapter; the UI never substitutes a nearby
placement or silently captures unrelated pending work.

Joint rows expose original-joint editing, drive, limits, suppression, endpoint
highlighting and confirmed deletion. Forms use native joint types/origins and
preserve the joint ID during reference repair. World-point, geometric-reference
and as-built controls are explicit. Native frame guides, degrees of freedom,
unresolved origins and solver diagnostics describe the real solve. Exact
capability limits are in [ASSEMBLIES.md](ASSEMBLIES.md).

Changing a choice/reference requests one context inspection with the complete
current draft. Token, form-version and exact-parameter checks reject older
callbacks. The matching response refreshes native DOF/units/frame defaults;
newer text remains untouched. Keystrokes do not send commands. If text changed
while context inspection was pending, the next explicit Preview refreshes those
complete current fields before evaluating geometry.

Drive sliders share the expression fields and keep one evaluation in flight plus
one replaceable latest proposal. Apply waits for the matching result. Interrupted
slider input restores its starting draft; cancellation never calls Undo. A
requested `75 deg` remains visible as requested text when native limits produce
`50 deg`; the feedback reports the actual result rather than rewriting the text
and implying the requested motion occurred. Stops, clamping and no-movement
reasons come from the native command reply. Finger navigation continues through
the existing viewport path.

## Input, accessibility and cost

`MetalCanvas` remains the sole contact producer. UIKit controls own their touches.
Compact sheets block canvas input and consume outside taps, including during a
draft requiring explicit cancellation. Docking changes the canvas frame, so
rendering/picking metrics and presented epochs move together. Direct interactions
cancel on layout/focus changes; numeric previews survive keyboard geometry changes.
The single shared Rust camera path remains; sketch snapping belongs to the Step 6
sketch tools rather than to the specialised Post radius fixture.

UIKit exposes labels, values, selected/disabled traits, property groups, modal
focus and accessible view/confirmation actions. **VoiceOver has not been verified.**
Browser cells are reused, document-derived rows are revision-cached, reads are
coalesced, and camera/hover frames do not fetch the document. Panel changes do not
evaluate or retessellate geometry. Rendering stays event-driven, with no idle loop.

## Runtime checklist — visual and hardware verification pending

Builds and deterministic checks do not verify actual layout or physical input.
Record observed device/driver coverage in [HANDOFF.md](HANDOFF.md). On an
authorised physical iPad, capture actual screenshots in light/dark,
wide/portrait/narrow and larger text while checking:

1. Launch the real fixture; select Post in Model. Confirm highlight, selected row
   and radius inspector. Pencil-pick Base/Post; confirm browser and multi-selection.
2. Activate Radius, open Properties, Edit radius, enter `6 + 6`, Preview and Apply.
   Confirm geometry, Undo and Redo. Start another edit, preview, then Cancel;
   confirm original geometry and unchanged history.
3. Preview `2 +`, correct the error and retain valid work. Exercise decimal commas,
   units, named parameters, text undo and visible fields above the keyboard.
4. Navigate with fingers; open/close panels, tap scrims, scroll Model and resize/
   rotate. Confirm no tap-through, unwanted edit or broken picking.
5. Check longer labels, rail placement, contrast, VoiceOver/focus order,
   background/foreground, Pencil suspension/rebase and optional interactions using
   [INPUT.md](INPUT.md).
6. In Assemble, create/activate a component, distinguish repeated nested paths,
   move a placement in world/parent coordinates, and Preview/Apply/Cancel. Check
   linked versus independent copies, ground state, isolation and scoped pending
   capture/revert. Confirm Undo/Redo and native Save/Reopen preserve structure.
7. Create/edit/drive a joint through its actual form. Change joint/type/DOF choices
   and verify refreshed controls; exercise raw expressions, a native limit stop,
   no-movement diagnostics, an unresolved origin and explicit reference repair.
   Verify Pencil handle editing with finger navigation, interrupted sliders,
   cancellation and confirmation without duplicate actions.

Mac GPU captures, synthetic controller drivers and human UIKit/Pencil checks are
distinct evidence. No checklist item here claims a hardware result.


## Project browser verification

Use the header folder menu > Projects. Create two projects, add distinct geometry,
switch between them, terminate/relaunch and confirm the last project returns.
Duplicate using a row context menu; confirm edits/Undo in the copy leave the
original unchanged. Delete an inactive project and cancel a second deletion.
Search, large text and keyboard input must remain usable in a resized iPad scene.

Export STEP to Files and import it into a new project; check geometry, placement
and component names. Also insert STEP into an existing design and Undo it. Export
an editable SolidCraft design, import that file, edit its timeline and save.
Cancel both pickers, try a damaged/empty file and confirm the current design
remains intact. Check iCloud/local providers, file extensions and repeated names.
Use Share STEP to open the system share sheet, cancel it, then reopen it and send
the file to an available destination. Confirm the received file has the project
name and `.step` extension. The opt-in DEBUG `--step-share-check` uses a temporary
fixture to check real STEP output, iPad popover anchoring, repeated sharing,
temporary-file cleanup and the existing Files export. Its activity completion
callbacks are synthetic; it sends no file externally. The report and three
UIKit captures are written as `step-share-*` files in Documents.
During a numeric preview, autosave must preserve the preview while writing only
the committed model. Suspend/relaunch with dirty work and verify the saved project.

The macOS Foundation harness covers library identity, safe deletion, bounded
staging and names; Rust tests cover native/STEP persistence, transactional failure
and autosave/preview isolation. Unsigned compilation is separate from the manual
UIKit/Files provider and physical-device checklist above.
## Feature history

The small History button at the bottom right starts collapsed on each workspace
launch. Tap it to show the full-width, horizontally scrollable feature strip;
tap its history icon at the right edge to collapse it. Navigation arrows stay on
the left and feature icons fill the space between the arrows and toggle. Features
appear as canonical icons in
engine order, with names retained only for tooltips and accessibility labels.
Suppression and selection retain their visual and accessibility states.
The strip keeps its current selection when collapsed and reopened. Empty designs
show an empty-history message when expanded. Selection is disabled while an edit,
plane choice or command owns the workspace.

The backward/forward arrows execute the shared `timeline.roll_to` command one
feature at a time. A blue divider marks the current history position; future
features are dimmed. Press and hold Back or Forward to jump to the beginning or
end. Tapping a future feature first advances the marker to include it, then
selects and edits it. Navigation is disabled during sketch or provisional editing. The
viewer displays the evaluated model at the marker, including an empty model at
the beginning. Existing inspector edits invalidate the downstream evaluations;
stepping forward or returning to the end rebuilds the later features from those
new inputs. Native save/reopen retains the shared marker and complete timeline.

Grab the blue history divider with Pencil or a finger and drag horizontally to
scrub between features. Its grab region extends around the thin line without
intercepting neighbouring feature taps. Dragging snaps at feature boundaries and
holding at either edge scrolls a long strip. The marker follows the requested
position while the existing activity status reports computation. One
`timeline.roll_to` evaluation runs at a time; intermediate samples coalesce to
the latest boundary, including the final position on lift. The authoritative
workspace acknowledges that position before scrubbing settles. VoiceOver users
can adjust the marker with increment/decrement actions.

Collapsing the strip, resizing, switching documents, starting an edit or
suspending the app cancels queued drag targets. An engine command already
dispatched may finish through the normal command path. No alternative undo
stack, evaluator, camera or document state is introduced.

Tapping a feature uses the same `select.entity` path as the model browser, waits
for the matching workspace state, then opens its existing native feature editor.
Sketch icons reopen the original sketch with `sketch.edit`. Apply retains the
feature identity and uses the shared evaluator; Cancel discards the draft.
Press and hold an icon for **Delete feature**. Deletion uses the existing
dependency warning and revision-checked confirmation, followed by
`timeline.delete`; Undo restores the feature and its geometry. An action from an
old document's menu cannot act on a replacement document.

Both surfaces reflect the viewport's feature IDs. Solid features highlight the current
bodies they created or modified, including placed component bodies. A modifying
feature such as a fillet highlights its current body. Sketch selection outlines
the solved source curves, including a consumed sketch; explicit item, folder and
occurrence visibility still applies. Construction features receive geometric
feedback. Clearing selection removes the highlights. Selection reuses the model
upload and leaves the document, camera and undo stack intact. History navigation
uses the engine's existing saved marker and undo semantics.

The native layout harness includes collapsed/expanded history with both chrome
styles, both rail sides, large text, the browser and keyboard-reduced windows.
`--history-check` is an opt-in DEBUG launch diagnostic on a physical iPad. It
uses a disposable fixture, presses the real toggle and feature buttons, checks
selection feedback, steps backward/forward and checks the visible body counts.
It writes `Documents/history-check-{collapsed,selected,browser,stage-1,stage-2,stage-3}.png`.
It does not load or autosave a library design. The render proof's `--history`
mode independently captures real GPU feedback for a box, cylinder, sketch and
fillet, checks visible pixel changes and exact restoration after clearing.

2026-10-29 verification: 406 native layout/focus/decoder cases and canonical icon
checks pass; 135 session and 28 bridge tests pass. The unsigned and existing-profile
device builds pass. The physical iPad diagnostic and GPU proof pass; their actual
captures were inspected. No simulator was used. Logs are local ignored output in
`target/ipados-feature-history-*.log`.

The subsequent marker-drag update passes the Foundation snapping, reversed drag,
pending-evaluation coalescing, final-lift and stale-completion checks. Unsigned
and existing-profile device builds pass. The physical DEBUG diagnostic also
passes the production coordinate/queue/command route for backwards, reversed and
forwards scrubbing, including the empty stage and final full model. Its captures
are inspected separately from hardware contact: it does not synthesize a Pencil
gesture. Hands-on Pencil feel and long-strip edge scrolling remain unverified.
Local logs use `target/ipados-history-scrub-*.log`.

`--history-actions-check` is an opt-in disposable device workflow for tapping an
existing box, applying a height edit without replacing its ID, opening a future
feature's editor, invoking the native Delete menu action and the same destructive
confirmation closure, undoing deletion, and reopening/finishing the original
sketch. It writes local `Documents/history-actions-*.png` captures. It does not
load, replace or autosave a library design, or synthesize a hardware long press.
The 2026-10-29 device run passes these editing/deletion/Undo checks. The unsigned
and existing-profile native builds, 550 native layout/focus/decoder cases,
canonical icon checks, pending locale lists and UI container checks pass. Local
build/runtime logs are `target/ipados-history-actions-*.log`.

History feature buttons use UIKit's semantic primary action for Edit and an
explicit cancelling long-press recognizer for an anchored Delete/Cancel action
sheet. Native button context-menu previews were reproduced hiding the feature
icon and raising the keyboard without displaying Delete on the physical iPad.
History items do not install that preview or the competing large-content viewer.
Their accessible name remains, with a custom Delete action for VoiceOver. The
history-actions diagnostic checks that the sheet retains the icon without opening
the editor or keyboard, then exercises the existing dependency confirmation,
deletion and Undo. Its calls alone do not verify a physical long press.

2026-10-29 history hold fix verification: 678 native checks and icon validation,
unsigned and existing-profile signed device builds, and the physical
`--history-actions-check` pass. A 1.1-second pointer hold through Device Hub on
the connected iPad also shows the anchored Delete action with its feature icon
still visible and no software keyboard; outside dismissal restores the normal
strip. This is remote pointer input, not a Pencil/finger hardware claim. The
device diagnostic capture is local `target/history-longpress-check/delete-menu.png`.
Normal app launch was restored after the disposable-model check.

## View controls and History labels

The separate View button shows its name and canonical chevron above the right
navigation controls (mirrored with handedness). Crowded windows place it below
the document header to keep both controls readable. Its View-only popover follows
the approved reference: a heading and Fit model row; Front/Back, Left/Right and
Top/Bottom pairs; a full-width Isometric choice; then Perspective, Grid and
Origin references switch rows and an Individual references disclosure. Standard
view selection is read from the actual Rust camera orientation and clears after
free orbit or roll. Opening the panel executes no command and preserves the
Metal view frame. All actions use the existing viewport command routes, including
the conditionally applicable planar-exit action. No text input or native
UIButton context menu is used by this panel.

The View pages use the existing canonical SolidCraft artwork beside their labels:
Fit, Perspective, Grid, Origin, Axis, Plane and Point. Standard views retain the
engine registry's shared Camera icon. Leading artwork remains visible alongside
the trailing selected-view checkmark or disclosure chevron, including after
appearance changes. Switches and mixed visibility retain their existing behavior.

Grid remains independent of Origin references. The latter includes **Axes,
planes & origin**: all visible hides all seven references; none or a mixed set
shows all. Mixed visibility shows **Some visible**, a neutral switch track and
a minus indicator, and announces the mixed value through VoiceOver. The native
host reads effective visibility, grid and projection from the existing Rust
viewport diagnostic, including automatic plane-choice visibility. Saved overrides
change only after command success. Edits and pending commands disable the
corresponding controls.

Individual references opens a second page in the same popover and retains the
origin point, per-axis and per-plane switches, axes/planes groups, Show all/Hide
all, Automatic, and the origin-in-model preference. Back returns to the View page.
All labels are localized; semantic colors, Dynamic Type and scrollable contents
retain light/dark and narrow-window support. The tool menus, model browser and
left rail retain their existing presentation. The History toggle keeps its
canonical rollback icon and localized label, measured width and original
timeline handlers; the separate hold fix is documented above.

The opt-in DEBUG argument `--view-controls-check` uses a disposable fixture,
restores visibility preferences and never opens or autosaves a library design.
It checks the actual panel buttons/switches, all/none/mixed and automatic
visibility, Grid independence, camera-derived standard selection, Perspective,
busy/failed commands, unchanged document/history/selection, and opening without
a keyboard or viewport/camera changes. It writes
`Documents/view-controls-{opened,none,all,mixed,panel}.png` for inspection.
The Foundation harness covers all 128 effective reference subsets, camera
selection including free orbit/roll, and adaptive label/control placement.

2026-10-29 reference-panel correction: 692 native harness cases and canonical
icon checks, pending locale lists, UI-container checks, unsigned device build and
existing-profile signed build pass. The final physical `--view-controls-check`
passes real button/switch dispatch, effective visibility/mixed/automatic states,
Grid independence, busy/failure preservation, standard-view selection and
projection changes. Opening preserves the camera, Metal frame and native
document/history/selection, and raises no keyboard. Actual physical captures
`target/task3-view-panel/{opened,none,all,mixed,panel}.png` were inspected against
the reference; remote pointer opening also displays the full panel without the
old native-menu keyboard issue. Hardware Pencil/finger and hands-on VoiceOver
coverage remain separate from these UIKit/remote-pointer checks. Saved visibility
preferences and the normal Test document were restored after the disposable run.

## Sketch constraint choices

The sketch Constraints button uses a text-free anchored action sheet, matching
the History workaround above. It retains the ten localized constraint choices,
disabled ineligible choices and Cancel. Opening executes no command; choosing an
action rechecks the current document, sketch, geometry revision and selection
before dispatching the existing Rust-authored eligible command. This avoids the
native UIButton menu preview that can unexpectedly raise the software keyboard
after selecting two sketch edges. The disposable `--constraints-menu-check`
diagnostic covers opening/reopening without text focus or canvas changes,
eligibility, Parallel and Undo; physical contact remains separate coverage.

## Grouped modelling options

The compact inspector keeps its source, operation and persistent action footer.
More options presents secondary choices in native grouped rows. Exact Boolean
choices use switches; direction choices keep their Rust values and use localized
labels. Target bodies appear as compact named rows with a remove control and,
when order can change, a contextual reorder menu. The single selection menu
offers unselected bodies and viewport selection. Busy previews disable discrete
controls; generation checks reject events from a superseded form.

The Foundation harness covers typed Boolean recognition and ordered reference
edits. The DEBUG-only `--grouped-inspector-check` fixture exercises the current
compact UIKit panel without changing a library project. Supply a fresh
`--grouped-inspector-run=<UUID>`; accept evidence only when the run matches and
the report ends with `PASS complete`. It checks menu actions, busy recovery,
stale controls, target callbacks and normal/accessibility layouts at 320/380pt.
This synthetic device check does not establish physical Pencil or VoiceOver
coverage.
