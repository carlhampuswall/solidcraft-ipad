# Consolidated iPadOS development branch

`develop` is the combined development branch for this fork. `main` remains the
upstream integration base at `0accea759d36127154b99478601b4ba8ff71c899`.
This consolidation follows the 2026-10-28 request to integrate completed work,
preserve existing branches and working changes, verify the application without
running the engine suite, and push only `develop`.

## Discovery and provenance

All origin branches were fetched without pruning. The fork contained only
`main` and `feature/ipados`; no existing `develop` or open pull requests were
found. Local branch, reflog, stash, registered worktree and Codex worktree
inspection found the source below. Object inspection found snapshot trees/blobs,
but no unreachable implementation commits and no stashes.

| Preserved source | Commit | Included work |
| --- | --- | --- |
| `origin/feature/ipados` | `0d548f650de65656ad519d60b1423d42ba2daf95` | Step 1 preparation |
| `feature/ipados` | `fc58c334d997e7514f96d6b44b753f272dad9e91` | Steps 2–3, including `dc96ee1484b723503595b1ff5f5e2ab69cf57a92` |
| Step 8.5 source baseline B | `c22acd12702cf3a405df01009b8d8662b3d46365` | Accumulated Steps 4–8, history gestures, integration fixes |
| `integration/ipados-foundation`, policy P | `380b43b2047a1242c81dfec75142283a0b61bd76` | Finalized Step 8.5 contracts, upstream exceptions and task policy |
| `codex/ipados-ui` | `22d4308ee1838f7786b2531f45e4f410f7c32447` | Floating CAD workspace and safe-area follow-up, including `10d35224` |
| Primary checkout's completed working files | Preserved before integration; committed on `develop` | Later tasks listed below |

The UI branch has the only separate SolidCraft Codex worktree:
`/Users/chw/.codex/worktrees/ipados-ui/SolidCraft - iPad`. It is retained with its
original branch and commits. The primary checkout already contained its UI
changes plus subsequent features. Those combined files are verified together;
the UI branch is also merged to preserve its real commit ancestry. Existing
branches are not rewritten, deleted or pushed. No unrelated upstream update is
imported, and the finalized B/P policy is retained rather than replaced by the
older planning text at B.

The completed working source is committed as
`05e6fabd8c1f64d2b464e4b6eb528fcea138c41d`. The UI history merge reviewed 17
overlapping hunks across six files. Xcode retains the canonical icon resources
and both Project source registrations. Workspace components retain icon-first
chrome plus mutable value-chip labels and exact entry. Controller/layout keep
floating safe-area geometry alongside origin controls, project menus, push/pull,
value-chip accessibility/cancellation and the bounded input mailbox. Handoff/UI
text retains original evidence and the later project service and B/P policy.
The merged source is byte-identical to the verified combined candidate; only
this integration report is extended after that comparison. No blanket merge
strategy, history rewrite or feature rollback is used.

The six initially unpushed commits are `dc96ee14`, `fc58c334`, `c22acd12`,
`380b43b2`, `10d35224` and `22d4308e`. The prior Step 8.5 inventory remains in
[CONSOLIDATION-INVENTORY.md](CONSOLIDATION-INVENTORY.md).

## Completed later tasks

The integration waited for these chats to finish, including the three extrusion
tasks started during discovery:

| Completed work | Chat |
| --- | --- |
| Floating workspace, original icon bridge, safe-area coverage | Overhaul SolidCraft iPadOS CAD UI — `01a183e0-2fce-7572-ab7c-0adfd07d8031` |
| Object-centred orbit and subsequent drag-direction correction | Fix single-finger rotation direction — `01a18480-44d4-72f2-9242-dff8e649b8de`; Build and install iPad updates — `01a18498-4718-7be1-96bc-f66547e56d7b` |
| Viewport sketch-plane picking, origin planes/axes/point | Add interactive sketch plane picker — `01a18481-7748-7042-9c26-d033c564d0f0` |
| Part-browser visibility and automatic sketch-creation visibility | Add plane visibility controls — `01a1849e-bad7-7da1-ba71-5026d26011a2` |
| Project library, autosave/restoration, native/STEP/mesh import/export | Add project browser and STEP I/O — `01a1849b-19fd-7230-a1a8-e3e98d67338f` |
| Exact closed-region Pencil selection for extrusion | Select enclosed sketches for extrude — `01a184aa-3c25-7711-9f72-59b1a5f9aa84` |
| Continuous Pencil resizing, bounded sample coalescing and safe re-grabs | Fix continuous extrusion resizing — `01a184aa-f890-7013-b5ac-a01e00503134` |
| Extrusion/push-pull axis arrows and exact numeric entry | Design extrusion handles and numeric — `01a184a9-bac6-7380-a7f6-b9e8c890d820` |

No separate completed performance or regression-workstream branch/worktree was
found. The older A–D prompts describe planned scope, not evidence that every
planned feature was implemented.

## Preservation and integration boundaries

The initial recovery checkpoint and branch bundle remain under
`target/develop-preservation/`. After all tasks finished, 37 changed/untracked
source files were archived in
`target/develop-preservation/final-completed-tasks/`, with a binary tracked patch,
per-file SHA-256 manifest and ref inventory. The final archive SHA-256 is
`a487ca139713a4cf8fc8341d0e1a2db813d87c32ca312b16364a05645e96bbca`.
Earlier `target/step8.5-preservation/` archives remain intact. Recovery archives,
build output and review images are local artifacts, not committed source.

Later functionality is confined to the native host, iPad Rust adapters and
pending locale files. CAD commands, persistent references, native containers,
history, solver and kernel stay upstream-owned. The shared changes already
accepted in B remain documented in [ARCHITECTURE.md](ARCHITECTURE.md). Rebuild
Rust and Swift together after JSON/ABI changes; the C ABI remains version 3.

Changes a default: yes. Existing preferences retain earlier presentation/input:
compact chrome, system appearance, history gestures, legacy touch orbit, legacy
menu plane chooser, explicit origin visibility, starter-model launch and the
cross/Pencil-only handle are still reachable. See [UI.md](UI.md),
[INPUT.md](INPUT.md) and [INTERFACES.md](INTERFACES.md) for exact controls and keys.
The final orbit mapping makes the object follow the drag; the historical
`follows_viewpoint` wire key is preserved for compatibility.

## Verification of the combined candidate

Verification results are recorded here before publication. Exact commands,
exit codes, durations and logs are retained in
`target/develop-verification/results.json`. A source fingerprint guards against
concurrent edits during the run. This is scoped development verification, not a
completed full upstream CI run.

The combined source passed on **2026-10-29 (Europe/Stockholm)** with Rust
`1.95.0`, the `ci` profile, Xcode 27 / iPhoneOS 27 SDK and minimum iPadOS 17.
All 3,303 source-file fingerprints matched before and after verification.
Only documentation was finalized after that run; its fast checks are repeated
before committing. No production integration repair was required.

| Check | Result |
| --- | --- |
| Formatting and diff whitespace | PASS |
| Assets, icons, licences, lists, skills, layers, UI containers | PASS |
| Workspace all-target Clippy, `--locked --profile ci -- -D warnings` | PASS |
| Session / bridge library tests | PASS: 125 session + 28 bridge, no ignored/filtered tests |
| Native Foundation harness | PASS: 330 adaptive cases plus production decoder, library, value-chip and bounded input-mailbox cases; 42 real icon mappings |
| Shared desktop UI library | PASS: 2,932; 14 existing ignored tests |
| Desktop application | PASS: 87; no ignored tests |
| iPad dependency guard | PASS: 13 shared crates / 177 resolved CAD packages |
| Full wasm compatibility check | PASS: 14 crates; five warnings in unchanged shared engine/UI sources remain |
| Web offline/update browser harness | PASS in Chrome with the harness's stand-in app; this is not a full web UI runtime build |
| Unsigned native application | PASS: arm64 Rust archive and Swift/UIKit application compile/link; no simulator launch |
| Mac GPU proofs | PASS: profile selection, orbit, modelling/push-pull, origin visibility, assemblies and continuous resizing; representative actual captures inspected |

The profile proof selects the right-hand 200 mm² region and produces only its
2,000 mm³ extrusion. Continuous resizing retains one synthetic Pencil contact
through 10→15→20→12 mm and applies 14,400 mm³ once. Assembly proof validates
four components, hinge/rigid/slider relations, 120→90 degree and 75→50 mm limits,
analytic world centroids, cancellation and native save/reopen. These complement
the adapter tests for original IDs, expressions, history, preview lifetime,
failed imports and native/STEP round trips.

Proof commands use the existing `render_proof` example with `--profiles`,
`--orbit`, `--modelling`, `--origin`, `--assemblies` and `--continuous-resize`.
PNG/native-design outputs remain in `target/develop-verification/`. The unsigned
product is `target/ipados-xcode/Build/Products/Debug-iphoneos/SolidCraft.app`.
No GitHub Actions run is claimed; the iPad workflow is PR/manual-triggered and
the inherited engine suite is not triggered by a direct push to `develop`.
Native file-coordination checks, Xcode resource compilation and the localhost
browser harness require an unrestricted macOS runner. A restricted post-merge
file-coordination retry could not open its synthetic STP fixture; that capability
failure is not counted as a source failure or a passing check. The successful
unrestricted run is retained in the verification log.

The engine suite and full `cargo xtask ci` are intentionally not run at the
user's request. No engine test, expectation, tolerance, golden or CI code is
disabled or changed. The three historical failures in
[KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) remain unresolved and
were not rechecked by this consolidation. They must be revisited for release
qualification or an explicit full-test request.

Mac GPU captures exercise real commands and renderer output; they are not UIKit
or physical Pencil evidence. Prior signed device installations and UI captures
predate the final combined source. Combined physical iPad/Files-provider,
Pencil/palm-rejection, VoiceOver and keyboard acceptance remains outstanding.
The renderer's broader dark-palette/grid refinement and older documented CAD
limitations are not claimed resolved by branch consolidation.

## Continuing development and upstream updates

Follow [TASK-COORDINATION.md](TASK-COORDINATION.md) for shared-directory work,
completed-batch checks and local commits on `develop`. Record the approved
local `develop` SHA and its relationship to observed `origin/develop`; preserve
locally integrated work when choosing the baseline. The B/P policy
remains the historical approval record for the original workstreams; it is not
an instruction to start new work from the older source after consolidation.
Use the primary checkout without per-task worktrees or a project build lock;
coordinate overlapping edits and shared native/session interfaces. Preserve
the patch ledger, old worktrees/evidence and upstream commit ancestry.

Continue reviewed upstream integrations through the fork's `main` using
[UPSTREAM.md](UPSTREAM.md), then merge the reviewed result into `develop` and
rerun affected checks. Do not reset `develop` to upstream, rewrite history or
move iPad dependencies into shared engine/kernel/document crates.
