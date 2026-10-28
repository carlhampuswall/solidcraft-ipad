# Native workspace — Steps 5–8

The UIKit host is a thin presentation layer over the real Rust session, viewport
and Step 4 input router. It adds no CAD document, evaluator, modelling registry,
camera controller, undo stack, dependency or artwork.

## Layout and reusable components

`WorkspaceController.swift` replaces the development toolbar. The top bar shows
the actual document name, real history availability and Model/Properties/View
access. The two-line subtitle identifies the active authoring component and exact
occurrence path, alongside the local-document status. The engine's `is_dirty`
projection distinguishes unsaved edits from context changes. Panels start closed.
Feature history and assembly placements share the Model browser. Save/Reopen local
design use the native container in the app's private storage; Files browsing,
autosave and restoration remain Step 9.

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
