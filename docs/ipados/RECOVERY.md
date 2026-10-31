# Last working save

Every native iPad project save now maintains `<project>.solidcraft.recovery`
beside the ordinary project and the engine's existing `.bak` file. This is the
same full native document container, including sketches, timeline, imports and
assembly data. It introduces no document format or shared-engine change.

The recovery file contains the latest successfully written document that also
passes the native scene checks after a fresh engine evaluation. Validation reads
the actual saved bytes and ignores the optional model cache and live sketch
editor state. This matters when a sketch edit leaves downstream references stale
or ambiguous: the normal save still preserves the unfinished edits, while the
recovery file stays at the last working version. Repeated saves cannot rotate
that version away. A failed recovery write reports a save error and leaves the
session unsaved so it can be retried.

On the first save after upgrading, an existing working project (or its working
`.bak`, if the main file fails validation) seeds recovery before replacement.
A newly created working project gets recovery on its first save. A document
that has never rebuilt successfully cannot supply a working recovery version.
Renaming a project moves its recovery file too, and a failed rename restores
both names. Project listing ignores recovery sidecars.

To recover manually, preserve the ordinary file and copy its `.recovery` file
to a new `.solidcraft` filename before importing it as a separate project. Keep
it beside the original if the design uses relative external component links.
The original is never automatically replaced.

## Open a project for repair

When ordinary opening reports `unresolved_reference`, the Projects alert offers
**Open for repair**. This opens a separate project copy, retaining the source and
its last working recovery file. Select the named feature in History, choose
**Edit original feature**, and explicitly reselect its inputs before applying.
The displayed evaluated geometry needs review: opening for repair does not
certify a fallback edge or profile as the intended input.

This path cold-evaluates the saved document without its optional model cache.
Only reference warnings may be displayed; evaluation errors and stale results
still refuse adoption. Modelling previews and commits retain their strict
reference checks. A successfully repaired save reopens normally and updates its
own working recovery file. No feature, parameter or history marker is silently
rewritten to make the project open. Relative external component links still
need review when moving a project into another library directory.

The second reported failure was a chamfer edge whose `c2#3` piece disappeared
after regeneration. The previous selected circular cut still evaluated, but the
edge lookup fell back to the nearest surviving piece. Strict opening turned that
repairable feature warning into a project lockout. The explicit repair path
removes that lockout without weakening the default opening or modelling checks.

## Investigation on 2026-10-31

The error visible on the physical iPad was `unresolved_reference` on an extrude:
its original sketch boundary was split by a later circle, and the shared engine
evaluated the two pieces inside the old boundary with a warning. The iPad's
strict snapshot checks refused that warning when opening the project. The ZIP
container and CRCs were intact; the current file and its rolling backup were
identical. This was a reference/authoring-state problem rather than truncated
file bytes. An active sketch editor can hide that downstream warning until the
document is reopened, which the recovery regression now exercises.

The original device projects were copied locally before diagnosis. A separate
repair copy uses `timeline.edit` to explicitly select both current regions. It
reopens without warnings and retains the original feature IDs, saved history
marker, and the same measured body volume. The source project on
the iPad was not replaced. Private source files and diagnostic artifacts remain
under ignored `target/recovery-investigation/`, outside Git.

Focused validation uses the existing `projects_` session tests and native
workspace harness. Full engine tests require an explicit request under
[TASK-COORDINATION.md](TASK-COORDINATION.md).
