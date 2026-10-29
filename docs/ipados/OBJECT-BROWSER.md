# Native object browser

The browser uses SolidCraft's canonical body, sketch, construction, component,
feature and folder artwork. Rows show a name, an eye where visibility applies,
and a disclosure control for containers. Details remain in the inspector.

- Tap the browser button to slide the browser in from the left at the button's
  original position. Other floating controls move to its right. Reduce Motion
  disables the animation. The unpinned viewport keeps its rendering/picking bounds.
- Tap an eye to show or hide an item or folder. Hiding a folder preserves its
  members' individual visibility choices.
- Press and hold an item to rename it, create a folder containing it, or move it
  into an existing folder. Press and hold a custom folder to rename or remove it;
  removing a folder keeps its contents in the design.
- Folders are scoped to a component and object type, as in the existing engine.
  The default Sketches folder is present in an empty design and receives new
  sketches.

All mutations use existing shared commands: `browser.group`, `browser.move`,
`browser.rename_group`, `browser.ungroup`, `browser.visibility`, `body.rename`,
`timeline.rename`, `component.rename` and `joint.edit`. Groups and visibility are
saved with the design and support Undo/Redo. Renaming a placed body targets its
source body; folder membership uses source keys, and visibility uses world names
and exact occurrence paths. Renaming a component placement renames its shared
component definition through `component.rename`.

The native preference **Open object browser from left** defaults to on. Turn it
off in More to restore the previous rail-anchored panel. The existing **Pin
browser** preference still controls whether the browser reserves viewport width.
The text-toolbar layout retains its existing panel policy.

## Find in model and aligned controls

The Model header's search button reveals **Find in model**. Names match without
case or diacritic differences; the count includes matching rows only. Results
retain their actual parent hierarchy and order, with collapsed ancestors revealed
temporarily. Clearing or dismissing search restores the previous expansion and
row-relative scroll position where valid. Escape dismisses search. Typing leaves
selection alone, and explicitly selecting a result keeps that selection afterwards.
Search focus owns text input and disables document shortcuts and viewport input.
Document replacement clears transient search state.

Search is a native presentation projection of the existing row IDs and parent
IDs. It sends no modelling, visibility or document commands. Duplicate names and
placed occurrences remain distinct. Long-press actions and delayed rename
confirmation recheck the current document, geometry revision and exact row ID;
stale actions are discarded. Existing busy/edit locks remain in force.

All rows reserve separate 44-point eye and disclosure columns, including leaves,
rows without eyes, and optional origin references. Missing controls leave inert
space. Indentation belongs to the name/icon area, while the original rounded eye
buttons retain their own visibility state and commands. A hidden folder still
preserves each child's individual visibility choice.

## Verification

Run the adapter/bridge tests, native layout checks and unsigned device build from
[BUILDING.md](BUILDING.md). Browser regressions live in the existing
`crates/ipados/session/src/tests.rs`; native layout regressions live in
`apps/solidcraft-ipados/scripts/workspace-check.swift`.

The opt-in DEBUG launch argument `--browser-check` uses a disposable fixture,
opens the browser with its real button, checks row icons and single-line content,
inspects long-press menu actions and taps a real eye control to hide/show a body.
It captures UIKit views to `Documents/browser-check-{dark,hidden,light}.png` and
prints its results to the device console. It does not load or autosave a library
design or synthesize a physical Pencil interaction. Capture files remain local
test output and are not committed.

The browser diagnostic also exercises actual search focus, collapsed ancestor
results, case-insensitive matching, no results, clearing, Escape, explicit result
selection, stale document/revision rejection, busy locks and UIKit eye-column
alignment at large text/narrow widths. Captures include `aligned`, `search`,
`search-post`, `no-results`, `large-text` and `search-light` stages. The Foundation
harness separately covers duplicate occurrence IDs, diacritics, rename/deletion
refresh and undo-equivalent restoration, malformed cycles, 10,000 rows and scroll
anchor clamping. These checks do not substitute for hands-on VoiceOver or Pencil
verification.

2026-10-29 task 4 verification: the native harness and icon checks pass (678
adaptive layout cases plus the browser projection/identity/scroll checks), as do
pending locale lists and UI-container checks. Unsigned and existing-profile
physical-device builds pass. The disposable device driver passes real filtered
eye hide/show, correct-object targeting, rename/Undo result refresh, explicit
selection, focus, Escape, locks and alignment checks. Normal dark/light search
captures were inspected. Rapid accessibility trait changes can leave transient
UIKit reuse artifacts in the diagnostic hierarchy snapshot; automated hit-column
checks pass, but hands-on VoiceOver and full large-text visual coverage remain
unverified. Logs and captures are local ignored files under
`target/task4-browser/`. No simulator, signing configuration change, commit or
push was used.
