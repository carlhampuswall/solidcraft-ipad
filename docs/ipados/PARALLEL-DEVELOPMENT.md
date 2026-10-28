# Parallel iPadOS development

**Ready for individually authorized parallel development.** Source baseline **B = `c22acd12702cf3a405df01009b8d8662b3d46365`** is committed and accepted under the bounded 2026-10-28 user exception in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md). Only those three unchanged engine failures may remain; all other required checks passed. CI retains its nonzero result and is not described as green. A future task still needs an explicit user launch request; no tasks have been launched and no branch has been pushed by this finalization.

Read launch policy from immutable documentation-only commit P, identified in the integrator's final report or resolved under the B/P verification rules below. Every source worktree starts at B. Do not use provisional planning documents inside B as the finalized policy.

- Repository: `https://github.com/carlhampuswall/solidcraft-ipad`
- Upstream: `https://github.com/tangency-labs/solidcraft`
- Integration branch: `integration/ipados-foundation`
- Exact approved starting commit: `c22acd12702cf3a405df01009b8d8662b3d46365`
- Prompts: [NEXT-TASKS.md](NEXT-TASKS.md)
- Provenance: [CONSOLIDATION-INVENTORY.md](CONSOLIDATION-INVENTORY.md)

## Baseline and worktrees

The verified source baseline and the finalized planning revision are separate.
Call the source SHA above **B**. Use the immutable planning commit **P** recorded
in the integrator's final report. If no P was supplied, resolve
`integration/ipados-foundation` (or its verified, authorized fetched remote ref)
to a full commit SHA only while it satisfies the checks below, then record P.
Read `docs/ipados/PARALLEL-DEVELOPMENT.md`, `NEXT-TASKS.md` and `HANDOFF.md` from P
with `git show P:path`, and verify that the finalized policy approves B. P must
be a documentation-only descendant of B. Subsequent integration-branch movement
never replaces recorded P. Stop if approval is missing or the revisions disagree.

Create each source worktree at B. Its older planning documents may still say
verification is pending: use the finalized documents read from P for launch
policy, not those provisional copies. Do not merge/check out P into task code,
use it as a different source baseline, or copy planning files over the worktree.
Record both B and P in the handoff. This avoids a document pretending to contain
its own Git commit hash while keeping every implementation on identical code.

Every task starts from the same approved full SHA, even if another task finishes
first. Do not use the moving integration-branch tip, `main`, a remote Step 1
branch, another task branch, or an uncommitted checkout as the starting point.
Do not reset, clean, overwrite or discard earlier task work. Inspect existing
branches/worktrees before creating anything; stop if a name/path is already in
use and ask the integrator to choose a nonconflicting location.

After an explicit task launch request, run from the existing repository using
the exact approved full source SHA below:

```sh
git cat-file -e c22acd12702cf3a405df01009b8d8662b3d46365^{commit}
git worktree add -b codex/ipados-files ../solidcraft-ipados-a c22acd12702cf3a405df01009b8d8662b3d46365
git worktree add -b codex/ipados-rendering ../solidcraft-ipados-b c22acd12702cf3a405df01009b8d8662b3d46365
git worktree add -b codex/ipados-ui ../solidcraft-ipados-c c22acd12702cf3a405df01009b8d8662b3d46365
git worktree add -b codex/ipados-regressions ../solidcraft-ipados-d c22acd12702cf3a405df01009b8d8662b3d46365
```

Create only the worktree needed for an approved task. In every task, record
`git rev-parse HEAD`, `git status --short --branch` and `git worktree list` before
editing. Use the explicit worktree directory for every command. A checkout on
another machine must fetch the approved commit from an authorized published
branch before starting; the local baseline is not available remotely until it
has actually been pushed.

Each worktree gets its own Rust `target/` and Xcode derived data. The established
`apps/solidcraft-ipados/scripts/build.sh` already sets both paths under that
checkout. Do not point `CARGO_TARGET_DIR` or Xcode derived data at another task's
directory. About 11 GiB was free at the blocked checkpoint, insufficient for
four independent clean native/workspace builds. Check capacity first; coordinate
heavy build slots or provision separate approved runners/storage. Source work
can be concurrent while builds run sequentially. Never delete someone else's
artifacts, change global toolchains/signing, or weaken checks to fit the machine.

## Exact file ownership

Ownership is exclusive unless a row explicitly says otherwise. A requested
change outside the assigned files is a dependency for the integration owner,
not permission to edit a shared file independently.

| Owner | Files allowed to change | Boundaries |
| --- | --- | --- |
| A — native files/documents | New `apps/solidcraft-ipados/SolidCraft/DocumentHost.swift`, `DocumentIO.swift`; new `apps/solidcraft-ipados/scripts/document-check.swift`, `check-documents.sh`; new `docs/ipados/FILES.md`; new `assets/locales/pending/ipados-files.json` | Native file services/lifecycle policy using injected existing `NativeSession`. No geometry, viewport, shared engine or app-entry edits. Final UI/lifecycle wiring and Xcode registration require the integrator. |
| B — rendering/performance | `crates/ipados/bridge/src/renderer.rs` including inline tests; `crates/ipados/bridge/examples/render_proof.rs`; new `docs/ipados/PERFORMANCE.md` | GPU resource/cache/render integration only. Existing shared renderer and immutable scene contracts remain authoritative. No sketch tools, file behavior or shared geometry algorithms. |
| C — UI/accessibility | `apps/solidcraft-ipados/SolidCraft/WorkspaceController.swift`, `WorkspaceComponents.swift`; layout/focus sections only of `WorkspaceLayout.swift`; `NativeText.swift`; `apps/solidcraft-ipados/scripts/workspace-check.swift`, `check-workspace.sh`; `docs/ipados/UI.md`; new `assets/locales/pending/ipados-ui-refinement.json` | Presentation, layout, accessibility and existing keyboard/pointer actions only. Freeze all DTOs, command envelopes, `PresentationJSON`, model response types and `ModellingPreviewQueue` in `WorkspaceLayout.swift`. Do not change command semantics, preview acceptance or shared input protocol. |
| D — integration/regression | `crates/ipados/session/src/tests.rs`; `crates/ipados/bridge/src/ffi/tests.rs`; `crates/engine/src/cmd/joints_tests.rs`, `occurrence_tests.rs`; `.github/workflows/ipados-build.yml`; new `.github/workflows/ipados-regression.yml`; new `docs/ipados/VERIFICATION.md` | Tests, scoped CI and validation records. Reuse existing test files and synthetic fixtures; do not alter production APIs to make tests pass. Coordinate any additional fixture files first. |
| Integration owner | `NativeSession.swift`, `Application.swift`, `MetalCanvas.swift`; bridge `src/ffi.rs`, C header and ABI roots; all session production files; all Cargo manifests/lockfile; Xcode project, xcconfig, `scripts/build.sh`; all `AGENTS.md`; `.github/workflows/upstream-sync.yml`; shared architecture/handoff/implementation and these parallel/prompt documents | Sole owner of cross-task APIs, app registration/lifecycle hooks, input routing, native session lifetime, dependencies, upstream changes and final integration. Unlisted shared files also require owner coordination. |

C owns `WorkspaceController.swift` during parallel work. A supplies a reviewed
file-service integration proposal; the integrator applies lifecycle/menu wiring
after C's changes are reviewed. A must not silently take ownership of that file.
B owns the render proof example, so D adds regressions in its existing test
modules or requests B's cooperation instead of modifying the example itself.

## Shared contracts

Use the actual types below; do not create parallel `EngineSession`, document,
history, camera, selection or tool implementations under different names.
Read [INTERFACES.md](INTERFACES.md), [ARCHITECTURE.md](ARCHITECTURE.md), [INPUT.md](INPUT.md), [UI.md](UI.md),
[SKETCHING.md](SKETCHING.md), [MODELLING.md](MODELLING.md) and
[ASSEMBLIES.md](ASSEMBLIES.md) for the detailed established behavior.

| Contract and owner | Consumers / requirements |
| --- | --- |
| `SessionAdapter` in `session/src/lib.rs`; underlying upstream `Session` | Sole mutable CAD document owner. Bridge dispatch and prepared transaction acceptance mutate it; native UI/renderers never borrow or duplicate its document. `SessionAdapter::execute` delegates existing commands. |
| `NativeSession.command(_:params:completion:)` and `BridgeError` in the native host | A/C use this existing boundary on the main thread. CAD commands serialize on its engine queue; view/input commands use its existing display lane. Results are delivered on main while the session remains alive. A keeps its own file-operation lifetime/cancellation bookkeeping and never assumes a callback will arrive after shutdown. Do not bypass the queue with raw ABI calls. |
| `CommandReply`, `WorkspaceState`, `SceneSnapshot`, `RenderSnapshot` | Document identity/revision, geometry revision and presentation epochs have distinct meanings. Read engine-projected dirty/history state. Scene/picking data must come from the same revision. UI caches are disposable; they cannot become authoritative saved state. |
| `Sel` and `Viewport` selection; occurrence paths | Retain native entity/feature IDs and complete root-to-occurrence paths. Labels, screen points, GPU indices and first matching names are not persistent references. Regeneration must resolve or invalidate selection safely. |
| `InputEvent`, `ToolWork`, `Viewport` in the session adapter | One source-aware ownership system; main-thread UIKit copies input, never passes live touches across workers. Points have top-left origin/Y-down; drawable pixels apply scale once. Camera navigation uses the existing shared path. UI controls consume their contacts; no second gesture/camera controller. |
| `NumericEdit`/`PreparedNumeric`, `ModelEdit`/`PreparedModel`, `PreparedTool` | Preview uses upstream scratch evaluation; commit accepts one current native transaction. Document/revision/token/version checks reject stale results. Cancel discards provisional work without undoing committed work. Model/assembly handle lift does not Apply. A last-good display is not a validated current preview. |
| `NativeRenderer`, immutable `Arc<SceneSnapshot>` and instance transforms | B may optimize resource reuse within this boundary. No engine mutation or geometry evaluation on the render lane. Render/picking coordinates and complete effective occurrence transforms must agree. Do not label a skipped draw as a presented frame. |
| ABI 3 and surface lifetime | Engine, viewport and surface response lanes serialize independently through length/copy. UIKit/layer geometry stays on main; the host retains `CAMetalLayer` until renderer destruction/replacement. The existing render-only lock serializes lifecycle suspension; it must not cover CAD evaluation. |
| Native document commands and `crates/io` | A delegates serialization/import/export to the checked-out engine commands, inspecting actual parameters first. Native file access/security scopes and coordination may be platform responsibilities; CAD bytes/history/format remain upstream. Preserve unexposed parametric data and explicit unsaved-change handling. |
| Error/progress callbacks and cancellation | Preserve structured codes/messages and existing activity state. Provisional evaluation may finish after cancellation; reject superseded preview acceptance/publication. Ordinary dispatched commands may finish while inactive; never claim interrupted computation or rollback. No success status before the relevant command/frame/save actually succeeds. |

Any new API proposal must name the affected files, callers, payload/version
changes, threading/lifetime/ownership, error/cancel semantics and compatibility
tests. The integrator establishes and reviews it before dependent implementation
or integration. If it changes the approved baseline contract, pause affected
tasks and issue one documented common contract update; do not let each task
invent its own adapter. Independent in-scope work can continue meanwhile.

## Validation and handoff rules

Read root/nested AGENTS and applicable repository skills. Use project Rust
`+1.95.0` and the established Xcode SDK/build script; no simulator launch. Check
capacity/load before heavy work and keep logs in that worktree's ignored target
directory. Use pnpm if JavaScript tooling is truly needed; do not add dependencies
where existing code suffices.

All changes require `git diff --check`, formatting and relevant tests. Rust work
requires affected Clippy with `-D warnings`. Run
`cargo +1.95.0 xtask ci` before every commit, without changing branches or files
during the run. A failed or unavailable mandatory gate must remain explicit:
preserve work and report the cause. Only the same three upstream failures
documented in KNOWN-UPSTREAM-FAILURES may remain under the user-approved
exception. Check both their names and underlying behavior against the baseline;
any new or changed failure blocks acceptance. This is not a green CI claim or
permission to suppress tests, widen tolerances or change golden expectations. Native changes require an unsigned
device-target build and relevant deterministic harness; hardware observations
are separate and require actual execution. No simulator testing is authorized.

Logical commits retain each task's history. Do not push, open/update a PR or
publish evidence without authorization. Prepare a reviewable handoff even when
publication is not yet authorized: starting SHA, commits or dirty diff, owned
files, behavior, exact test commands/results/logs, public API proposals, upstream
delta, screenshots/recordings for observable changes, limitations and blockers.
Follow the repository PR template, including `Changes a default:`; retain the
old behavior behind a preference if a default changes. Target the integration
branch, never `main`, for this workstream review. Check existing PRs/drafts first
and do not create a duplicate claim.

## Dependency order and final integration

1. Establish shared contracts and approve all coordinator-owned changes first.
2. Integrate D's baseline-compatible test/CI harness after review. Future tests
   that require A/B/C behavior wait for their dependency; a red test-only commit
   is not an independently mergeable result.
3. Review B and C independently. They can integrate in either order when neither
   requires a contract change; rerun their shared viewport/input regression set.
4. Review A's isolated file service, then let the integrator wire registration,
   lifecycle/session hooks and the current UI. Run D's cross-module document,
   sketch/modelling/assembly and persistence regressions on the combined result.
5. Run full repository CI, native build, desktop/web compatibility and available
   runtime workflows on the exact final candidate. Record device/automated/build
   coverage separately before approving the next baseline.

A's isolated service/tests and B/C/D can run concurrently within their ownership
and resource limits. A's final app wiring depends on integration-owner changes;
tests for later feature behavior depend on that behavior landing. Order is based
on these dependencies, not completion time. No completed task is auto-merged.

Use ordinary reviewed merges into the integration branch, preserving unrelated
feature histories. Cherry-pick only isolated recovery work with its provenance
recorded. Do not rebase or force-push shared work, switch a branch beneath a
running gate, or delete source branches/worktrees. For conflicts, inspect both
changes and the contract/behavior they preserve; consult the file owner, resolve
the smallest necessary change and rerun affected tests. Never use blanket
`ours`/`theirs`. Broader upstream updates remain separate reviewed integrations
under [UPSTREAM.md](UPSTREAM.md), not part of these task merges.
