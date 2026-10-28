# Future Codex task prompts

**Resumed with a bounded validation exception (2026-10-28).** The user asked
that the three already-reported upstream engine failures be left unchanged and
tracked for later review. [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md)
records their exact scope. A nonzero CI exit must remain visible; all other
checks must pass. Final source commit and launch policy are still pending here.

**NOT APPROVED FOR LAUNCH.** `BASELINE_PENDING_VERIFICATION` is deliberately not
a Git commit. The integration owner must finish required checks, commit the
consolidation and replace every marker here and in
[PARALLEL-DEVELOPMENT.md](PARALLEL-DEVELOPMENT.md) with the same approved full SHA.
Until then these prompts are drafts, not executable authorization. Do not use
current HEAD as a substitute. No future implementation tasks were launched.

Each prompt is self-contained. Start an approved task with its recommended model
and reasoning effort, in its own worktree. Review disk/build capacity first:
About 11 GiB was free at the blocked checkpoint, insufficient for four
independent clean native/workspace build directories. Coordinate heavy build slots or use approved separate runners/storage.

## Task A — Native Files and Document Management

```text
Use GPT-6.1 Sol with Medium reasoning.

Repository: https://github.com/carlhampuswall/solidcraft-ipad
Upstream: https://github.com/tangency-labs/solidcraft
Integration branch: integration/ipados-foundation
Exact approved starting SHA: BASELINE_PENDING_VERIFICATION
Branch: codex/ipados-files
Worktree: ../solidcraft-ipados-a (relative to the original repository)

Before creating the worktree, use immutable policy SHA P from the integration
owner's final report. If none was supplied, resolve the integration branch (or
its verified authorized fetched remote ref) only while it satisfies the checks
below. Record P; branch movement never replaces it. Read its
docs/ipados/PARALLEL-DEVELOPMENT.md, NEXT-TASKS.md and HANDOFF.md using
git show P:path. Verify that they approve the exact source SHA above (B), and
that P is a documentation-only descendant of B. Stop if these checks fail.
Create code at B; do not merge/check out P or use it as another code baseline. B may retain draft
planning documents: use the finalized policy from P for approval and current
handoff instructions, not the provisional copies in B. Record both B and P.

Validation exception: read docs/ipados/KNOWN-UPSTREAM-FAILURES.md. Run every
required gate without disabling tests. The full CI command may exit nonzero
only for the same three documented upstream engine failures with unchanged
underlying behavior. Record that failure explicitly; do not call CI green.
Any new/changed failure blocks acceptance. Do not alter goldens or tolerances.
This bounded exception also applies to the commit/testing instructions below.

STOP before implementation if the SHA above is still a marker, is unavailable,
or is not approved by the finalized policy at P. Do not substitute HEAD,
main or another task branch. Inspect status/branches/worktrees, preserve all
existing work, and create your own fresh worktree/branch at that exact commit.
If the name/path already exists, ask the integrator to resolve the collision.
Record starting HEAD. Do not run in the integrator's or another task's checkout.

Read AGENTS.md and scoped iPad instructions, docs/ipados/{UPSTREAM,SETUP,BUILDING,
ARCHITECTURE,HANDOFF,PARALLEL-DEVELOPMENT,INTERFACES,KNOWN-UPSTREAM-FAILURES,MODELLING,ASSEMBLIES}.md, and applicable
skills for contribution, commands, file formats, crash safety, performance and
tests. Use existing Rust 1.95.0 and Xcode tooling; do not change global defaults,
signing credentials or dependencies. Do not launch an iOS/Android simulator.

Implement native Files/document services: open, save/save-as and export; import
only formats actually supported by checked-out engine commands; safe coordinated
file access, security-scoped URL lifetimes, autosave/recovery and document
lifecycle policy. Use existing upstream serialization and safe-write behavior.
Preserve parametric history, expressions, feature/component/joint identity and
unexposed native data. Do not implement a new document format or geometry logic.

Your exclusive files are new:
- apps/solidcraft-ipados/SolidCraft/DocumentHost.swift
- apps/solidcraft-ipados/SolidCraft/DocumentIO.swift
- apps/solidcraft-ipados/scripts/document-check.swift
- apps/solidcraft-ipados/scripts/check-documents.sh
- docs/ipados/FILES.md
- assets/locales/pending/ipados-files.json

Do not modify NativeSession.swift, Application.swift, MetalCanvas.swift,
WorkspaceController.swift, Xcode project/xcconfig/build.sh, Cargo manifests/lock,
AGENTS.md, the ABI/header, any session production module or shared CAD source.
Do not change viewport/rendering, input ownership, modelling tools or undo rules.
The integrator owns registration and application/menu/lifecycle wiring; Task C
owns UI presentation. Supply a concrete integration proposal and coordinate it.
Implement/test isolated file services through an injected existing NativeSession
while wiring is pending. Do not report user-facing integration complete until
the owner has wired and verified it. Do not create a replacement session adapter.

Use NativeSession.command(_:params:completion:) on main as the existing command
boundary; inspect exact file/import/export command parameters first. It queues
CAD work serially and returns Result<Void, BridgeError> on main while alive.
WorkspaceState projects native document ID/dirty state; it is not a second model.
Document replacement must invalidate draft/input/revision context through owner-
provided existing hooks. Preview state must never be autosaved as committed CAD.
Keep file access valid until asynchronous I/O completes; cancellation/shutdown
must release ownership safely even if native callbacks are discarded. Escalate
any missing result/lifecycle contract to the integrator before depending on it.

Acceptance: supported native designs open/save/reopen and remain editable;
exports/imports use actual supported APIs; cancelled picking or I/O does not
replace the active document; failed save retains prior data and truthful dirty
state; recovery cannot silently overwrite a newer user file; coordinated access
and security-scope cleanup cover success/error/cancel; active previews and stale
callbacks cannot save or publish into another document. Unsupported formats and
provider failures produce structured errors. Define autosave scheduling against
actual committed revisions and document identity, not render frames.

Add meaningful deterministic tests in your document harness for operation order,
identity/revision checks, cancellation, provider failures and recovery policy.
Coordinate real engine/ABI persistence regressions with D instead of editing D's
test files. Once owner registration is available, run the harness and unsigned
device build: sh apps/solidcraft-ipados/scripts/build.sh. Also run the existing
workspace harness, adapter/ABI tests, dependency guard and applicable wasm check.
Run git diff --check and cargo +1.95.0 fmt --all -- --check, then mandatory
cargo +1.95.0 xtask ci before every commit. Do not commit a newly failing gate or change
branches/files while it runs. Report resource/environment blockers explicitly.
Each worktree uses its own target/derived data; never share CARGO_TARGET_DIR.

Distinguish compilation, deterministic tests, real Files provider/device tests
and human interactions. Capture actual evidence for visible changes where
authorized runtime access permits; do not fabricate or claim device acceptance
from mocks. Do not launch a simulator or change signing.

Make logical commits only when repository gates permit. Do not push or open a
PR without authorization. Handoff to the integrator with starting SHA, branch,
commits/dirty status, exact changed files, contract requests, required app wiring,
test commands/results/log paths, real evidence, upstream delta and limitations.
Prepare a draft PR description against integration/ipados-foundation including
Changes a default: and recovery behavior; check existing PRs before publication.
Do not merge, delete branches/worktrees, force-push or rebase shared work.
```

## Task B — Rendering and Performance

```text
Use GPT-6.1 Sol with High reasoning.

Repository: https://github.com/carlhampuswall/solidcraft-ipad
Upstream: https://github.com/tangency-labs/solidcraft
Integration branch: integration/ipados-foundation
Exact approved starting SHA: BASELINE_PENDING_VERIFICATION
Branch: codex/ipados-rendering
Worktree: ../solidcraft-ipados-b (relative to the original repository)

Before creating the worktree, use immutable policy SHA P from the integration
owner's final report. If none was supplied, resolve the integration branch (or
its verified authorized fetched remote ref) only while it satisfies the checks
below. Record P; branch movement never replaces it. Read its
docs/ipados/PARALLEL-DEVELOPMENT.md, NEXT-TASKS.md and HANDOFF.md using
git show P:path. Verify that they approve the exact source SHA above (B), and
that P is a documentation-only descendant of B. Stop if these checks fail.
Create code at B; do not merge/check out P or use it as another code baseline. B may retain draft
planning documents: use the finalized policy from P for approval and current
handoff instructions, not the provisional copies in B. Record both B and P.

Validation exception: read docs/ipados/KNOWN-UPSTREAM-FAILURES.md. Run every
required gate without disabling tests. The full CI command may exit nonzero
only for the same three documented upstream engine failures with unchanged
underlying behavior. Record that failure explicitly; do not call CI green.
Any new/changed failure blocks acceptance. Do not alter goldens or tolerances.
This bounded exception also applies to the commit/testing instructions below.

STOP if the SHA is still a marker, unavailable or unapproved by
the finalized policy at P. Never substitute current HEAD/main/another
task branch. Inspect status/branches/worktrees and create a separate fresh
worktree at the exact approved SHA; do not overwrite existing names or work.
Record starting HEAD and use that worktree explicitly for every command.

Read root/scoped AGENTS, docs/ipados/{UPSTREAM,SETUP,BUILDING,ARCHITECTURE,HANDOFF,
PARALLEL-DEVELOPMENT,INTERFACES,INPUT,MODELLING,ASSEMBLIES}.md and relevant contribution,
crash-safety, performance, command-boundary, UI and testing skills.

Improve measured viewport performance, GPU resource efficiency, renderer-side
work scheduling and large-model responsiveness within the existing native GPU
adapter. Profile before changing behavior; define reproducible workloads and
compare resource/upload/draw costs. Preserve output correctness and interaction
semantics. No new renderer, geometry kernel, evaluator or tessellation algorithm.

Your exclusive files:
- crates/ipados/bridge/src/renderer.rs (including its inline tests)
- crates/ipados/bridge/examples/render_proof.rs
- docs/ipados/PERFORMANCE.md (new)

Do not modify session production APIs, ffi.rs/header, NativeSession.swift,
MetalCanvas.swift, Application.swift, UI components, document services, shared
GPU/geometry/kernel sources, manifests/lock, Xcode configuration or AGENTS.md.
Do not alter sketch tools, document-management behavior, input/preview protocol,
camera math or dirty/history semantics. If host scheduling or a shared renderer
API must change, submit the measured need and minimal contract proposal to the
integrator first; continue independent in-scope work while awaiting it.

NativeRenderer consumes immutable RenderSnapshot/Arc<SceneSnapshot>; the engine
owns CAD evaluation. Scene/picking data must share document/geometry revisions,
and complete native occurrence transforms remain authoritative. Reuse upstream
Renderer, SceneSlot and mesh-instance APIs. Maintain distinct model, overlay,
selection and camera invalidation; no stale geometry/picking agreement claims.
Surface calls share the existing render-only lock and lifecycle suspension.
Never evaluate CAD under that lock, submit Metal after suspend, release a layer
before surface destruction or label DrawOutcome.presented=false as a frame.
Preview/commit/cancel and last-good-display semantics are fixed contracts.

Acceptance: representative unchanged-scene/camera/selection/preview/assembly
workloads remain visually and geometrically correct; measured optimized paths
reduce the intended cost without growing caches unboundedly; resource lifetimes
survive resize/suspend/resume/surface-loss recovery; repeated occurrences retain
correct transforms and selection; cancelled/stale previews cannot reappear.
Report device and sample conditions, baseline/change measurements and variance.
Do not invent an FPS target or claim hardware latency from compile-only results.

Put targeted cache/lifetime/invalidation regression tests inline in renderer.rs.
Coordinate broader test additions with D. Run cargo +1.95.0 test --locked
--profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib;
affected all-target Clippy --no-deps -- -D warnings; cargo +1.95.0 fmt --all -- --check;
git diff --check; the dependency guard and applicable wasm checks; unsigned
sh apps/solidcraft-ipados/scripts/build.sh. Use the real render_proof example to
produce and inspect actual before/after model evidence for changed output, with
the established fixture modes documented in BUILDING.md. A Mac Metal capture is
not an iPad/UIKit screenshot. Run mandatory cargo +1.95.0 xtask ci before every
commit and report any failure rather than suppressing it. No branch/file changes
while the gate runs.

Use project Rust +1.95.0/Xcode; no global toolchain/signing changes or simulator
launches. Keep target/derived data local to this worktree. Coordinate heavy build
slots: the consolidation machine had about 11 GiB free, insufficient for four
independent clean native/workspace build directories. Never share CARGO_TARGET_DIR or delete another task's data.

Commit logical changes only after required gates. Do not push/open PRs without
authorization. Deliver starting SHA, branch, commits/dirty status, measured
findings, exact files, API requests, tests/logs/evidence and remaining risks.
Prepare a PR against integration/ipados-foundation with Changes a default: and
the performance experiment details. Check existing PRs before publication.
Do not merge, force-push, rebase shared history or delete task worktrees/branches.
```

## Task C — UI Refinement and Accessibility

```text
Use GPT-6.1 Sol with Medium reasoning.

Repository: https://github.com/carlhampuswall/solidcraft-ipad
Upstream: https://github.com/tangency-labs/solidcraft
Integration branch: integration/ipados-foundation
Exact approved starting SHA: BASELINE_PENDING_VERIFICATION
Branch: codex/ipados-ui
Worktree: ../solidcraft-ipados-c (relative to the original repository)

Before creating the worktree, use immutable policy SHA P from the integration
owner's final report. If none was supplied, resolve the integration branch (or
its verified authorized fetched remote ref) only while it satisfies the checks
below. Record P; branch movement never replaces it. Read its
docs/ipados/PARALLEL-DEVELOPMENT.md, NEXT-TASKS.md and HANDOFF.md using
git show P:path. Verify that they approve the exact source SHA above (B), and
that P is a documentation-only descendant of B. Stop if these checks fail.
Create code at B; do not merge/check out P or use it as another code baseline. B may retain draft
planning documents: use the finalized policy from P for approval and current
handoff instructions, not the provisional copies in B. Record both B and P.

Validation exception: read docs/ipados/KNOWN-UPSTREAM-FAILURES.md. Run every
required gate without disabling tests. The full CI command may exit nonzero
only for the same three documented upstream engine failures with unchanged
underlying behavior. Record that failure explicitly; do not call CI green.
Any new/changed failure blocks acceptance. Do not alter goldens or tolerances.
This bounded exception also applies to the commit/testing instructions below.

STOP if the SHA remains a marker or is unavailable/unapproved by
the finalized policy at P. Do not substitute HEAD/main/a task branch.
Inspect existing Git state, preserve work, and create a separate fresh worktree
at that exact commit. If its branch/path exists, coordinate before reusing it.
Record starting HEAD and work only in your own checkout.

Read root/nested AGENTS and docs/ipados/{UPSTREAM,SETUP,BUILDING,ARCHITECTURE,
HANDOFF,PARALLEL-DEVELOPMENT,INTERFACES,KNOWN-UPSTREAM-FAILURES,UI,INPUT,SKETCHING,MODELLING,ASSEMBLIES}.md; use the
repository's contribution, UI/text, preferences, commands, performance and tests
skills. Keep original/openly licensed assets and pending message catalogs.

Refine tablet layout, toolbars/inspectors, responsive presentation, accessibility,
keyboard/pointer access and visual consistency. Preserve existing modelling,
assembly and document behavior. Prefer reusable native components and semantic
colors/Dynamic Type. Prioritize concrete observed usability/accessibility defects.

Your exclusive files:
- apps/solidcraft-ipados/SolidCraft/WorkspaceController.swift (presentation only)
- apps/solidcraft-ipados/SolidCraft/WorkspaceComponents.swift
- apps/solidcraft-ipados/SolidCraft/WorkspaceLayout.swift (layout/focus only)
- apps/solidcraft-ipados/SolidCraft/NativeText.swift
- apps/solidcraft-ipados/scripts/workspace-check.swift and check-workspace.sh
- docs/ipados/UI.md
- assets/locales/pending/ipados-ui-refinement.json (new)

Freeze WorkspaceLayout.swift DTOs, EngineCommandEnvelope, PresentationJSON,
ModellingPresentation/Response, assembly DTOs and ModellingPreviewQueue. Do not
change command dispatch, preview validation/acceptance, engine algorithms, the
input protocol, camera/selection ownership, document services or history. Do not
edit NativeSession.swift, Application.swift, MetalCanvas.swift, Rust production
or D's test files, project/xcconfig/build.sh, manifests/lock, AGENTS.md or shared
architecture/handoff docs. A must coordinate file-menu integration through the
integrator after your UI work; do not wire an unreviewed parallel file service.

Use existing WorkspaceState and native presentation DTOs, NativeSession callbacks
and command(_:params:completion:) on main, and existing tool/numeric forms.
Do not create a second document/selection store or infer commands from labels.
Keep feature IDs/full occurrence paths opaque and stable. UIKit owns control
contacts; the existing MetalCanvas/InputEvent path owns viewport gestures.
Do not add competing tap/pan/pinch producers. Keyboard text undo remains native;
document shortcuts respect current text/draft ownership. Layout changes must
preserve the existing cancellation/metrics path. Native geometry stays in canvas
points and drawable scaling remains owned by the existing viewport boundary.

Acceptance: controls remain usable at supported widths/orientations, safe areas,
keyboard sizes and accessibility text sizes; content scrolls instead of losing
actions; readable labels and logical accessibility focus/order/hints reflect
actual enabled state; pointer/keyboard actions reach existing commands without
accidentally editing geometry; focus/layout changes do not apply/cancel another
draft unexpectedly. Light/dark/system presentation uses existing theme semantics.
Any changed default retains its old behavior behind an existing or owner-approved
preference and is declared in the handoff/PR. No speculative new tool behavior.

Extend the existing native layout/focus harness with meaningful cases and run
sh apps/solidcraft-ipados/scripts/check-workspace.sh. Run unsigned device build
via sh apps/solidcraft-ipados/scripts/build.sh, adapter/ABI tests, git diff --check and cargo +1.95.0 fmt
--all -- --check. Run mandatory cargo +1.95.0 xtask ci before every commit; no
branch/file changes while running and no silent skips. Capture real screenshots
or recordings for observable UI changes where authorized runtime permits and
inspect them. Distinguish deterministic layout checks, compilation, device UI
inspection and actual VoiceOver/Pencil/keyboard testing. Do not claim accessibility
hardware coverage from static checks; record missing coverage.

Use established Rust +1.95.0/Xcode, no global environment/signing changes and no
simulators. Use only this worktree's target/derived data; coordinate heavy builds
because four independent build trees exceed the recorded free capacity. Never share CARGO_TARGET_DIR.

Commit logical changes only after validation meets the bounded exception policy. Do not push or open/update a PR
without authorization. Handoff starting SHA, branch/commits/dirty status, exact
files and behavior, shared API requests, tests/logs, genuine visual evidence and
remaining coverage. Prepare an integration-branch PR with Changes a default:;
check existing PRs first. Do not merge, force-push, rebase shared work or remove
other task branches/worktrees.
```

## Task D — Automated Integration and Regression Testing

```text
Use GPT-6.1 Sol with High reasoning.

Repository: https://github.com/carlhampuswall/solidcraft-ipad
Upstream: https://github.com/tangency-labs/solidcraft
Integration branch: integration/ipados-foundation
Exact approved starting SHA: BASELINE_PENDING_VERIFICATION
Branch: codex/ipados-regressions
Worktree: ../solidcraft-ipados-d (relative to the original repository)

Before creating the worktree, use immutable policy SHA P from the integration
owner's final report. If none was supplied, resolve the integration branch (or
its verified authorized fetched remote ref) only while it satisfies the checks
below. Record P; branch movement never replaces it. Read its
docs/ipados/PARALLEL-DEVELOPMENT.md, NEXT-TASKS.md and HANDOFF.md using
git show P:path. Verify that they approve the exact source SHA above (B), and
that P is a documentation-only descendant of B. Stop if these checks fail.
Create code at B; do not merge/check out P or use it as another code baseline. B may retain draft
planning documents: use the finalized policy from P for approval and current
handoff instructions, not the provisional copies in B. Record both B and P.

Validation exception: read docs/ipados/KNOWN-UPSTREAM-FAILURES.md. Run every
required gate without disabling tests. The full CI command may exit nonzero
only for the same three documented upstream engine failures with unchanged
underlying behavior. Record that failure explicitly; do not call CI green.
Any new/changed failure blocks acceptance. Do not alter goldens or tolerances.
This bounded exception also applies to the commit/testing instructions below.

STOP if the starting SHA is still a marker, unavailable or unapproved by
the finalized policy at P. Never substitute moving HEAD/main/another
task branch. Inspect status/branches/worktrees, preserve all work, and create a
separate fresh worktree at the exact approved SHA. Resolve name/path collisions
with the integrator. Record HEAD and use only your own checkout.

Read AGENTS.md/scoped instructions and docs/ipados/{UPSTREAM,SETUP,BUILDING,
ARCHITECTURE,HANDOFF,PARALLEL-DEVELOPMENT,INTERFACES,KNOWN-UPSTREAM-FAILURES,INPUT,SKETCHING,MODELLING,ASSEMBLIES}.md;
apply contribution, testing, crash-safety, commands, file-format and performance
skills. Use existing project Rust +1.95.0/Xcode. No global toolchain/signing
changes, new dependencies without demonstrated need, or simulator launches.

Improve automated end-to-end engine/adapter integration coverage, meaningful
sketch/modelling/assembly regressions, native build verification, scoped CI and
upstream compatibility checks. Inspect existing tests first; avoid duplicating
implementation or making tests that pass only by inspecting their own fixtures.

Your exclusive files:
- crates/ipados/session/src/tests.rs
- crates/ipados/bridge/src/ffi/tests.rs
- crates/engine/src/cmd/joints_tests.rs
- crates/engine/src/cmd/occurrence_tests.rs
- .github/workflows/ipados-build.yml
- .github/workflows/ipados-regression.yml (new, narrowly scoped)
- docs/ipados/VERIFICATION.md (new)

Use existing area test files and in-memory synthetic/analytic fixtures; obtain
owner agreement before adding fixture files. Do not modify production interfaces,
session/renderer/native code, ABI/header, Cargo manifests/lock, AGENTS.md, Xcode
configuration/build.sh or upstream-sync.yml. B owns render_proof.rs; C owns the
native workspace harness. Coordinate there rather than editing their files.
Do not suppress an inherited failure, change expected values to current output,
introduce a knowingly red test-only commit or refactor production to fit a test.
Report reproducible defects and proposed minimal fixes to the integration owner.

Tests must use existing SessionAdapter/Viewport and ABI 3 boundaries, upstream
Session/commands/history, immutable revisioned SceneSnapshot, native references
and the current preview/commit/cancel contracts. Verify cross-module behavior:
- Sketch edits regenerate dependent solids with analytic dimensions/volume.
- Original feature IDs survive editing, cancellation and undo/redo.
- Selection resolves or safely invalidates after regeneration/document change.
- Camera/finger navigation cannot commit geometry or revive a stale preview.
- Component transforms, nested occurrence paths and joints agree with engine
  state, grounding/limits and expected analytic poses.
- Native save/reopen preserves supported parametric data, expressions and IDs.
- Cancelled/superseded work, stale revisions and duplicate confirmation remain
  safe across controller/ABI boundaries with deterministic synchronization.

Acceptance: added tests catch demonstrated missing behaviors/regressions, pass
on the approved baseline or on a coordinated fix, and exercise real engine paths;
CI documents what it actually builds/tests and covers native plus portable
boundaries without signing or launching simulators. Preserve upstream sync and
desktop/web compatibility. Workflow changes use repository action pins and least
permissions; local validation is distinct from an actual GitHub Actions run.
Do not claim a workflow ran before its exact revision has a real run.

Run focused session/bridge tests with cargo +1.95.0 test --locked --profile ci
-p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib and focused shared
joint/occurrence tests. Run affected all-target Clippy --no-deps -- -D warnings,
fmt, git diff --check and workflow lint where available. Run the dependency
guard, cargo +1.95.0 xtask wasm, cargo +1.95.0 xtask web-offline-test, desktop
all-target check and unsigned sh apps/solidcraft-ipados/scripts/build.sh as applicable to your CI changes.
Run mandatory cargo +1.95.0 xtask ci before each commit. Record exact commands,
exit results/logs, filtered/skipped tests and unavailable environments. Do not
change branches/files during a gate or call focused checks the full gate.

Use per-worktree target/derived data and coordinate heavy builds: about 11 GiB
was free at the blocked checkpoint, insufficient for four independent clean
native/workspace build trees. Do not share CARGO_TARGET_DIR, delete others' data or omit required gates.
Compilation, deterministic engine tests, browser tests, physical runtime and
human Pencil/VoiceOver checks must be reported separately.

Build baseline-compatible harness work first. A/B/C feature-dependent tests
wait for the relevant owner-reviewed behavior and contract; do not pull moving
task branches or silently rebase onto another starting point. The integrator
chooses dependency-aware integration and reruns your suite on the combined tree.

Commit logical changes only when gates permit. Do not push, publish a workflow
run or open/update PRs without authorization. Handoff starting SHA, branch,
commits/dirty status, exact files, covered failures, API/fix requests, test logs,
CI execution versus validation, remaining risks and a prepared PR description
against integration/ipados-foundation with Changes a default:. Check existing
PRs before publication. Do not merge automatically, force-push, rebase shared
work, delete branches/worktrees or import unrelated upstream updates.
```
