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
menu opens Projects, Save project, Import and Export. The separate top-left
Model button opens the model browser. Projects presents the native design library
with search, create/open, and context-menu duplicate/delete actions. Switching
saves first; current projects cannot be deleted. Imports can open as a separate
project or insert geometry into the current one. Export offers STEP, SolidCraft,
STL, OBJ and 3MF through Files. See INTERFACES.md for ownership and limits.

The floating viewport fills the entire app window, including the space behind
the status and home indicators. Only the controls are inset to the safe area;
there are no reserved black bands above or below the model. The workspace itself
also fills the window, so the extended viewport remains a real native input
surface. Pinned browsers retain their horizontal reservation. The keyboard still
reduces the usable viewport height through the existing resize/cancellation path.

The 52-point tool rail (56 for accessibility text) contains icon-first controls
with at least 44-point interaction regions. Modelling groups expose the actual
native-supported create, modify, pattern/mirror and assembly forms. Additional
native-supported operations remain reachable in the create menu. Sketch editing
replaces these with Line, Rectangle, Circle, Arc, Spline, Trim, overflow tools,
Finish chain and Finish sketch. Constraints, dimensions and construction appear
in the opposite rail only while sketching. Constraint menu eligibility and
dimension types come from `eligible_constraints` / `eligible_dimensions`; the UI
does not decide geometry compatibility or construct new constraint semantics.

View orientation, Fit, projection and Properties use the existing viewport
commands. Active sketches omit unavailable 3D orientation/projection actions.
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
owned by the existing session and forms.

`WorkspaceLayout.resolve` uses safe-area display points, window size, Dynamic
Type, keyboard intersection and visible control counts. `placed(in:viewport:)`
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

Changes a default: yes — compact floating chrome and dark native chrome are the
new defaults. **Tools → Compact floating workspace** off restores the earlier
text rail, header and docking policy (`SolidCraft.workspace.compactChrome`).
**Tools → Appearance → System** restores the earlier appearance policy; an
existing `SolidCraft.workspace.appearance` value is retained. Rail side and
two-/three-finger history gesture preferences are unchanged. Pinning and diagnostic
visibility are transient native presentation preferences, never CAD data.

## Rendering integration still required

The supplied assembly screenshot was produced by the opt-in DEBUG
`--assembly-check` fixture and its explicit `view.top` command. It is the real
shared shaded/depth-tested renderer. It is not a temporary drawing or a second
2D CAD engine. The normal fixture is shown in an actual shaded 3D view on the
physical iPad with body highlighting and native picking.

Two platform presentation decisions explain the remaining white/dense-grid look:
`SessionAdapter::snapshot` calls `view::scene_with_guides`, which uses the shared
light `Palette::DEFAULT`; `Viewport::guides` always builds a grid with that palette.
`NativeRenderer` clears using the scene background. Native UIKit appearance cannot
correct those decisions. This UI branch deliberately leaves the owner-controlled
rendering and session files intact. It does not simulate a dark viewport with a
tinted overlay or advertise unsupported display settings.

The integrator/rendering owner should provide a display-only appearance/guide
seam in session `lib.rs` / `viewport.rs` and bridge `renderer.rs` if needed:
reuse `view::scene_with_palette_and_guides` and canonical theme tokens, keep mesh
buffers/revisions, picking, camera and transaction ownership intact, suppress the
ordinary 3D grid by default, and retain the planar grid/axes during sketching.
Wire `applyAppearance` to that seam through `NativeSession` only after its contract
is reviewed. Theme/grid changes must not dirty the document, change geometry
revision, retessellate CAD, or accept/cancel a draft. Verify shared shading/edges,
depth and face/edge highlights in both themes on the combined candidate.

## Verification and integration

The actual unsigned device build and a signed physical-iPad build pass. The signed
build reuses the pre-existing development certificate/profile without provisioning
updates. Real Device Hub screenshots and click/keyboard checks exercise browser
open/close, native body selection, radius Preview/Cancel and sketch entry/tool
switching. These are physical UIKit observations, not mockups; hardware Pencil,
palm rejection and VoiceOver are separate outstanding coverage. See the UI section
of HANDOFF for current evidence and final gate status.

Compatibility switching also has an opt-in DEBUG UIKit regression:
launch with `--chrome-check` to cycle text/floating layouts in light/dark,
then read `Documents/workspace-chrome-check.json` from the app container. It checks
actual visible icon/touch frames after UIKit layout, including the reparented
browser button and retained scroll offsets. It also checks that the actual
canvas fills the top/bottom window edges, that edge hit tests reach the canvas,
and that header/status controls stay safe. It does not merely repeat the
Foundation policy calculation. Four physical cases pass. The optional
`--chrome-check-restore-dark` restores compact/dark preferences after this check;
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

The Part browser includes an expandable **Origin references** group with the
origin point, X/Y/Z axes and XY/XZ/YZ planes. Each row and its eye button toggle
the reference's saved visibility without selecting or changing CAD geometry.
The group eye menu (also available in View) offers Show all, Hide all and reset
to **Automatic during sketch creation**. With no saved override, references are
hidden during ordinary modelling and temporarily appear in the sketch-plane
chooser. Cancel/creation restores normal visibility; active sketch axes remain
part of the sketch guides. Explicit per-item choices survive app relaunch and
take precedence over automatic visibility. These rows are native presentation
controls, separate from the authoritative document rows and their parent IDs.


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
During a numeric preview, autosave must preserve the preview while writing only
the committed model. Suspend/relaunch with dirty work and verify the saved project.

The macOS Foundation harness covers library identity, safe deletion, bounded
staging and names; Rust tests cover native/STEP persistence, transactional failure
and autosave/preview isolation. Unsigned compilation is separate from the manual
UIKit/Files provider and physical-device checklist above.
