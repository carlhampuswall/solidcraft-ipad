# Step 8.5 discovery and preservation inventory

Discovery date: 2026-10-28. This records the state found before consolidation;
current validation and the eventual approved baseline belong in [HANDOFF.md](HANDOFF.md)
and [PARALLEL-DEVELOPMENT.md](PARALLEL-DEVELOPMENT.md). Historical test results
below are not a claim that a later integration candidate has passed.

## Resumed consolidation addendum — 2026-10-28

The original discovery and checkpoint records below remain historical. After
preserving the blocked checkpoint, the user resumed Step 8.5 and accepted only
the three recorded upstream engine failures for a development baseline. Exact
names, measurements and upstream links are in
[KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md). Tests and expectations
remain intact, full CI is not relabeled green, and any new failure blocks
approval. Fresh full-CI validation and source baseline B are still pending;
the known three alone no longer block the development-baseline commit.

Additional completed user work is preserved from “Add multitouch undo and redo,”
chat `01a183dd-c2f4-74a1-a93d-024ef30a9e7c`: two-finger double-tap Undo and
three-finger double-tap Redo. Its files are
`apps/solidcraft-ipados/SolidCraft/MetalCanvas.swift`, `WorkspaceController.swift`,
`docs/ipados/INPUT.md` and new
`assets/locales/pending/ipados-history-gestures.json`. It calls existing engine
history, defaults on and offers the `SolidCraft.workspace.historyGestures`
preference to restore prior behavior. Its unsigned build passed in
`/tmp/solidcraft-history-gestures-build.log`; the existing 186 native layout
cases passed. Physical gesture verification remains outstanding. These are
separate-task results. The resumed combined candidate subsequently passes an
unsigned native build and all 186 Foundation checks in
`/tmp/solidcraft-step85-resume-native-{build,check}.log`. Its native English
catalog now has 373 messages, including the added history-gesture label.

“Overhaul SolidCraft iPadOS CAD UI” remained read-only while waiting for an
approved baseline; no overhaul implementation is included. No Tasks A–D were
launched by consolidation. Keep both the original and later checkpoint archives;
they precede this gesture work and do not represent the latest full candidate.

## Location and Git provenance

All eight milestones were located in the same local checkout:
`/Users/chw/Documents/ChatGPT/SolidCraft - iPad`. No inaccessible workspace or
missing implementation must be recovered. Steps 4–8 contain real source changes,
not only documentation.

| Ref at discovery | Full SHA | Meaning |
| --- | --- | --- |
| `feature/ipados` / initial HEAD | `fc58c334d997e7514f96d6b44b753f272dad9e91` | Completed Step 3 commit plus uncommitted Steps 4–8 |
| `origin/feature/ipados` | `0d548f650de65656ad519d60b1423d42ba2daf95` | Only Step 1 had been pushed |
| `main` / `origin/main` | `0accea759d36127154b99478601b4ba8ff71c899` | Fork base; left unchanged |
| Cached `upstream/main` | `ee6ec09b0a6d76ff614cc16da492072dce25b35f` | Previously fetched upstream tip; not merged by consolidation |

Remote inspection confirmed that the fork contains only `main` and
`feature/ipados` at the SHAs above, and the GitHub PR search returned no PRs.
`origin` remains `https://github.com/carlhampuswall/solidcraft-ipad.git` and
`upstream` remains `https://github.com/tangency-labs/solidcraft.git`.

`git worktree list --porcelain` found one registered worktree and no detached
HEAD. Branch/reflog inspection found the linear Step 1 → Step 2 → Step 3 history,
with Steps 2–3 unpushed. There were 24 modified tracked files and 23 untracked
implementation/documentation files, no staged changes and no stashes.
`git fsck --full --no-reflogs --unreachable` found only the empty tree, with no
unreachable implementation commits. No stashes were applied or dropped.

The Codex turn-diff ref
`refs/codex/turn-diffs/captures/1793203280496/93b57f38-e8d1-4512-b3dd-9e51345a0a70/base`
points to tree `18218bd8a8c34e237f31607c4846fe11950a047f`. It is an app snapshot
tree, not a commit or another feature history. Do not manufacture milestone
commits from it or use it as the parallel-development baseline.

The integrator created `integration/ipados-foundation` from the existing Step 3
HEAD with the dirty changes preserved. No merge or cherry-pick was necessary:
the later milestones had already been developed sequentially in this checkout.
Their changes overlap, so the original independent Step 4–8 commit histories do
not exist. Preserve the three real commits and record the accumulated work
honestly rather than inventing historical commit boundaries.

## Per-step implementation

Paths in the table are relative to the repository. `session/` and `bridge/`
abbreviate `crates/ipados/session/` and `crates/ipados/bridge/`; `native/`
abbreviates `apps/solidcraft-ipados/SolidCraft/`. Later milestones extend many
earlier files; this table identifies ownership and interfaces, not disjoint diffs.

| Step | Original source / SHA | Source files and interfaces | Functionality actually present | Historical verification and incomplete coverage |
| --- | --- | --- | --- | --- |
| 1 — fork and upstream compatibility | `feature/ipados`, `0d548f650de65656ad519d60b1423d42ba2daf95` | `.github/workflows/upstream-sync.yml`, root Cargo exclusions/AGENTS, platform READMEs, `SETUP.md`, `UPSTREAM.md` | Fork preparation, isolated platform locations, reviewed upstream integration process and optional draft-PR workflow | Actionlint, unchanged Cargo metadata and eight workflow scenarios reported. No app implementation/full build. Workflow activation and fork CI administration remained follow-ups. |
| 2 — native foundation | Same branch, `dc96ee1484b723503595b1ff5f5e2ab69cf57a92` | Native Xcode project, `Application.swift`, `NativeSession.swift`, `MetalCanvas.swift`; `session/src/lib.rs` (`SessionAdapter`); bridge C ABI/renderer; dependency guard; iPad build workflow | UIKit lifecycle, one Rust engine session, existing command execution and engine-generated geometry through the shared GPU renderer | 13 tests, unsigned device build, targeted Clippy/wasm and Mac Metal proof reported; later physical install/launch verified. Full CI failed. Small shared `crates/doc/src/walk.rs` iterator compatibility patch. |
| 3 — viewport and selection | Same branch, `fc58c334d997e7514f96d6b44b753f272dad9e91` | `session/src/{input,picking,viewport}.rs`, `SceneSnapshot`, native host/ABI/renderer; shared engine navigation exports and `crates/gpu/src/capture.rs` | Real bodies, model edges and sketches; one camera path; topology selection/highlights; revisioned updates and history integration | 28 adapter/ABI tests, shared navigation/capture tests, native builds and Mac GPU evidence reported. Earlier synthetic physical workflow passed; final device recheck deferred by user. Actual Pencil testing remained outstanding then. Full CI failed. |
| 4 — Pencil and multitouch | Uncommitted atop Step 3 | `session/src/input.rs`, new `editing.rs` (`ToolWork`, `PreparedTool`), native `MetalCanvas`/`NativeSession`, bridge/header, input locale and `INPUT.md` | Source-aware contact ownership, finger navigation/Pencil suspension and rebase, cancellation, bounded immutable previews, real Post-radius feature edit with one history commit | 43 focused tests, targeted Clippy/wasm, unsigned build and six Mac GPU captures reported. Physical simultaneous input/palm acceptance remained pending. |
| 5 — tablet interface | Same accumulated work | New native `WorkspaceController.swift`, `WorkspaceComponents.swift`, `WorkspaceLayout.swift`; new `session/src/workspace.rs` (`NumericEdit`, `PreparedNumeric`); native deterministic harness, project registrations and locale | Adaptive panels, actual model browser and contextual inspector, expression input, real preview/Apply/Cancel and undo/redo | 41 session + 12 bridge tests, 176 deterministic layout cases and unsigned build. Physical layout/accessibility coverage remained incomplete. |
| 6 — sketching and constraints | Same accumulated work | New `session/src/{sketching,sketch_input}.rs`; existing editing/input/workspace/viewport/ABI/native files; sketch locale and `SKETCHING.md` | Native sketch planes/lifecycle, lines/rectangles/circles/arcs/fit splines, snapping, solver-aware handles, constraints/dimensions, trim/extend/transforms, native history and serialization | Logs confirm 59 session + 15 bridge tests, 176 native cases, native/wasm/web-offline and affected Clippy. Analytic prism regeneration 12,000 → 18,000 mm³, undo/redo and native roundtrip. Physical iPad unavailable during final acceptance. Secondary unexposed variants remain documented. |
| 7 — modelling and timeline | Same accumulated work | New `session/src/{modelling,references}.rs` (`ModelEdit`, `PreparedModel`), existing adapters/forms, modelling locale and `MODELLING.md` | Extrude, fillet/chamfer, shell/revolve/hole/Booleans/patterns/mirror/sweep/loft; original-ID feature edits, dependencies/regeneration, named expressions, native local Save/Reopen | Logs confirm 82 session + 19 bridge tests, native build, affected Clippy, wasm/web, 176 native cases, six Mac GPU captures and CLI roundtrip. Physical modelling acceptance remained outstanding. Advanced options are explicitly unexposed. |
| 8 — assemblies and joints | Same accumulated work | New `session/src/{assembly,assembly_references}.rs`; existing model/input/scene/native files and GPU instance cache; shared `cmd/component.rs`, `cmd/joints.rs` and their existing regression files; assembly locale and `ASSEMBLIES.md` | Components/ownership/copies, exact occurrence paths, activation, placement/capture, seven joint families, original-joint edits/repair, limits/drive/reach, native diagnostics/history/persistence | Logs confirm 105 session + 23 bridge tests, 163 joint and 77 occurrence tests, 186 native cases, native/desktop/wasm/web/CLI checks. Physical production-driver workflow and human selection/finger navigation passed. User confirmed stable Pencil rotation/cancel after flicker fix. Full CI remained red. |

The Step 8 shared API additions are optional occurrence scoping for
`component.capture_position` and optional A/B origin rebinding for `joint.edit`.
Omitted parameters preserve existing behavior. They avoid capturing unrelated
pending placements or deleting/recreating an existing joint. They do not add a
solver, kernel or document format. See [ARCHITECTURE.md](ARCHITECTURE.md) for the
downstream patch ledger and compatibility maintenance.

## Codex provenance

All chats below used the same checkout. They were read for provenance, not
treated as fresh authorization to repeat earlier device or publication actions.

| Step | Chat title | Chat ID |
| --- | --- | --- |
| 1 | Prepare SolidCraft iPadOS branch | `01a17ec1-74fe-7853-95ff-d7bd8e465f75` |
| 2 | Build iPadOS Native Foundation | `01a17ed2-f83f-7c13-ab88-747759e67b22` |
| 3 | Build Real CAD Viewport | `01a17efb-02c5-7d51-b079-afbe68697411` |
| 4 | Implement iPad Pencil Multitouch | `01a18113-fbda-78b0-a2fe-f0bc11483337` |
| 5 | Build Tablet-First CAD Workspace | `01a18152-5a7b-7be3-b8c9-a9567a205e5b` |
| 6 | Implement iPad Sketch Editor | `01a18170-a4a5-7620-b46b-8db94946c5fe` |
| 7–8 | Implement iPadOS Parametric Modeling | `01a181b1-aa3f-7b71-98ff-d1e4ff463078` |

Step 8 is the later turn `01a18245-a90d-7772-b3ac-2039b2e36c33` in the Step 7
chat; there is no separately titled Step 8 chat to recover. Its messages
corroborate the physical input observations and the final Pencil flicker repair.

## Existing verification evidence and limits

Local `/tmp/solidcraft-stepN-*` artifacts were present for Steps 3–8. Examples:

- Steps 6–8 `adapter.log`, `native-build.log`, `native-check.log`, `clippy.log`,
  `ci.log`, wasm/web logs and inspected Mac GPU PNGs.
- Step 7 `solidcraft-step7-model.solidcraft` and six model captures.
- Step 8 `solidcraft-step8-engine-{joints,occurrences}.log` and
  `solidcraft-step8-device-final.log`; the latter records complete production
  form/preview/Apply/cancel/history/persistence and matching-frame checks.
- Step 8 `solidcraft-step8-ipad-runtime.solidcraft` and
  `solidcraft-step8-ipad-cli-compare.log`: four closed bodies, 6,280 mm³,
  original feature IDs 3/6/9/10 and analytical hinge30/slider50 poses. The Mac
  fixture `solidcraft-step8-assembly.solidcraft` is separate, at 5,056 mm³.

These are machine-local, temporary evidence, not versioned CI artifacts. Some
old filenames contain intermediate failures: Step 3 Clippy/CI logs and Step 5
Clippy do not by themselves prove the later targeted passes described in chat.
Candidate verification must be run again and recorded separately. No
`solidcraft-step2*` temporary files were found; Step 2 evidence remains in its
chat and committed historical documentation.

At discovery, full CI stopped on inherited `clippy::nonminimal_bool` in
`crates/geom/src/fill_general.rs:166`; a separate engine Clippy run also reported
`component_clone.rs` and `sketch_modify.rs` lints. Steps 4–8 were therefore left
uncommitted. Passing focused checks is not a passing workspace gate.

Unfinished coverage/scope is explicit: physical sketch/modelling acceptance,
simultaneous Pencil/finger/palm behavior, VoiceOver, measured performance,
advanced modelling options, advanced assembly relation/coupling/study editors,
and ambiguous repeated nested joint endpoints. Full Files/autosave/recovery is
future work. Historical Step 8 device success does not establish all of these.
Raw CoreDevice screenshots have a documented beta orientation/cropping defect;
they are not complete UIKit-layout evidence.

## Preservation and recovery

Before integration edits, the integrator saved
`target/step8.5-preservation/` (gitignored):

- `history.bundle`, verified as a complete Git bundle;
- `tracked.patch` and the empty `staged.patch`;
- `working-files.tar.gz`, containing all 47 modified/untracked source and
  documentation files from discovery;
- `manifest.json`, with the original HEAD and SHA-256 for every saved file;
- `status.txt`, `refs.txt` and `reflogs.txt`.

Existing source branches and the worktree remain intact. The archive is a local
recovery aid, not a remote backup or verified integration commit. If recovery is
needed, first copy these artifacts to a separate safe location, verify the
bundle and manifest, and inspect/extract the archive into a new directory.
Recover history into a separate checkout and compare the saved files there;
never extract over ongoing work, reset the original checkout or apply patches
blindly. No missing Step 2–8 implementation needs to be recreated.
