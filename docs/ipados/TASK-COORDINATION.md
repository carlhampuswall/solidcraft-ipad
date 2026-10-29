# Independent tasks, integration and iPad testing

Each task owns implementation, integration and testing through completion. No
permanent coordinator or background automation is needed. Use the standard-library
Python helper [task_protocol.py](../../scripts/ipados/task_protocol.py); linked
worktrees share its task records and resource locks in the Git common directory.
This is the current workflow authorized by the user on 2026-10-29. The older
[B/P workstreams](PARALLEL-DEVELOPMENT.md) remain historical provenance.

## Authorization and boundaries

The user authorizes separate local worktrees, local commits of each task's own
reviewed changes and local merges into develop as part of task delivery. This
is the explicit exception to the general no-commit-without-request instruction.
Pushes, public PRs, merges into main, upstream integration, history rewrites
and discarding another task's work still require separate authorization.
Permanent rules in [UPSTREAM.md](UPSTREAM.md), scoped AGENTS files and required
checks remain in force. The user explicitly replaced full CI before every
local task commit/merge with the scoped validation policy below. This does not
waive failures in a required check or alter upstream/main/release requirements.

## Start and work independently

From the primary checkout, first inspect branch/status/worktrees, then run:

    python3 -I scripts/ipados/task_protocol.py start feature-name

The helper creates codex/feature-name and an isolated worktree beneath the
primary checkout's ignored target/task-worktrees/, from the recorded local
develop commit. Use the returned absolute worktree path for all further work.
An explicit new --path is supported. Existing task names, branches and paths
are never overwritten. Physical iPad verification is required by default.
For a task with no device impact, explicitly justify opting out:

    python3 -I scripts/ipados/task_protocol.py start tooling-name \
      --ipad not-needed --reason "Development tooling only; no app behavior changes."

Start refuses to branch while the develop checkout has uncommitted work.
It never captures that work as a baseline. Preserve and attribute the existing
shared-checkout changes before consolidation; do not auto-stash, reset, clean
or commit them with a blanket git add. Until consolidation, tasks depending
on that work are **baseline pending**. An existing independent codex/ worktree
can register itself with adopt TASK and the same iPad options; this records
its merge base and never moves or copies its changes.

The local develop tip is authoritative for local delivery. The record also
captures the locally observed origin/develop ref, when present; that is not a
claim of a fresh fetch. Preserve local commits ahead of the remote. Remote
synchronization and upstream updates remain separate from task coordination.

Keep Rust output, Xcode derived data and test evidence inside the task's own
ignored target/. Do not share build directories or edit another checkout.
Closely coupled changes should be sequential tasks, or share an agreed API;
use a dependency note when work requires another task to integrate first.

## Commit, integrate, then finish

Review the task diff and run the helper's affected-area checks before committing:

    python3 -I scripts/ipados/task_protocol.py check feature-name --wait 60

Then stage explicit owned paths and commit locally. Freeze source files during
checks. Use Python 3.11+ on PATH for repository icon tools; Xcode's Python 3.9
does not provide their TOML reader. Relevant native build and deterministic
harness commands are in [BUILDING.md](BUILDING.md). Existing
user-approved exceptions apply only within their original scope and must be
reported; a historical failure is not automatically a current exemption.

From the task's clean worktree and recorded branch:

    python3 -I scripts/ipados/task_protocol.py integrate feature-name --wait 60 \
      --check "python3 -I scripts/ipados/test_task_protocol.py"

The optional --check is repeatable for additional affected-area checks. Commands
are parsed as argument lists, without a shell; use a task-owned script for
multi-step checks. Validation is selected from the actual changed paths:

- Routine documentation/tooling tasks run repository consistency checks and
  relevant protocol tests, without the CAD engine suite.
- Native UI tasks additionally run the unsigned device-target build and
  deterministic workspace harness. Adapter changes additionally run session
  and bridge tests and Clippy. Physical-device observations remain separate.
- Shared crates, desktop/web code, dependency/workspace configuration and
  unfamiliar paths require full cargo +1.95.0 xtask ci. The conservative
  fallback prevents a new shared area from silently receiving weaker checks.
- Before main integration or release qualification, run check TASK --full.
  These checkpoints retain full CI even for an otherwise narrow task. The
  helper itself only merges into develop and never publishes a release.

The integration helper:

1. Takes the shared integration lock and checks that develop is clean.
2. Merges the latest develop into the isolated task when necessary, preserving
   both histories. It leaves conflicts in the task worktree for review.
3. Runs the selected affected/full gate and extra checks on the combined candidate. A failed
   check or source/index change prevents the develop update.
4. Commits a clean merge candidate only after review and checks, then
   fast-forwards the clean develop checkout. If develop is not checked out,
   it updates the ref with a compare-and-swap against the verified base.
5. Verifies task ancestry and records the integrated candidate SHA and checks.

A conflict or failed check leaves the branch and diagnostic record intact.
Review both changes, resolve only the necessary conflicts, run the required
checks before committing the resolution, then retry integrate. If a failed
check left a merge in progress, finish that reviewed merge first; the helper
does not silently abort or commit a pre-existing Git operation. No blanket
ours/theirs, automatic stashing, force-updating occupied branches or resetting
develop. A moved baseline requires a new combined candidate and checks.

After required device testing (or a justified opt-out), run:

    python3 -I scripts/ipados/task_protocol.py finish feature-name

Finish requires a clean task checkout, its exact verified tip in develop,
and a passing device session for that tip when required. New task commits
invalidate completion until integrated and tested again. Report **done** only
after this command succeeds, with the integrated SHA and coverage. Otherwise
report **integration pending** or **testing pending** with the blocker. A
completed implementation or queued resource request alone is not done.

## Reserve the physical iPad

Integrate first and release the integration lock before waiting for the iPad.
Prepare any signed build in the shared build slot before reserving the device:

    python3 -I scripts/ipados/task_protocol.py build feature-name --wait 60 -- \
      sh target/build-signed-candidate.sh

Use one reservation for the entire install, launch, test and capture session:

    python3 -I scripts/ipados/task_protocol.py ipad feature-name --wait 60 \
      --device "$SOLIDCRAFT_DEVICE" --evidence target/ipad-session.txt -- \
      sh target/test-ipad-candidate.sh

The task-owned test script must install this worktree's prepared build, verify the intended
candidate, run acceptance scenarios, capture evidence, write the named fresh
nonempty evidence file, and exit successfully only on a pass. The helper exports
SOLIDCRAFT_CANDIDATE, SOLIDCRAFT_TASK and SOLIDCRAFT_DEVICE; use and record them.
Existing signing permissions and settings still apply.

For interactive computer-use testing, replace the command after -- with
--hold (before any --). Run it in a live terminal session, wait for
IPAD RESERVED, keep that process alive while using the UI and writing evidence,
then send pass or fail followed by a newline to release. EOF, interruption,
failure, missing/old evidence or changed source cannot record a passing result.
Do not install, launch, terminate, drive or capture the app outside a reservation.
After release, the next task acquires the device itself.

The device result identifies the tested commit. Related tests can run together
when that candidate covers their acceptance criteria, but each task must retain
its own results. Compilation, deterministic harness, synthetic device input and
physical Pencil observations are separate coverage levels; document each
honestly. A nonempty evidence file records the tester's result, not an automatic
proof of hardware behavior. No simulators without an explicit user request.

## Shared state, contention and recovery

    python3 -I scripts/ipados/task_protocol.py status
    python3 -I scripts/ipados/task_protocol.py note feature-name \
      "Blocked on another-task's committed selection API."

Status lists ownership, worktree/branch/base/tip, validation scope, checks,
blockers, results and active integration/build/iPad owners. This replaces routine broadcasts. Direct messages are
for blockers or contract changes within the user's authorization to message the
other chat. No continuous polling or permanent coordinator.

Locks use OS-backed flock files shared by linked worktrees. They are held by
the executing task/session, not by a daemon or a timestamp lease. A directly
launched child inherits the reservation, so killing its wrapper does not free
the device while that command still runs. Do not launch detached background
device jobs. Never remove lock files: unlinking them can create two lock owners.
Owner JSON is diagnostic only and may outlive an interrupted session; the OS
lock determines whether a resource is busy.

Checks acquire one shared build slot. Other heavy builds use the build command.
Children default to SOLIDCRAFT_TEST_JOBS=2 and CARGO_BUILD_JOBS=2, preserving
explicit caller overrides. This bounds parallel work without suppressing tests.
Acquire integration before build when both are needed, and release both before
requesting the iPad. Do not nest same-task helper commands inside a held iPad
session; prepare builds first. Check machine load before starting heavy work.

Waits are bounded to 0–60 seconds (default immediate). On contention, keep doing
independent work and retry when ready. Pending tasks and active owners are
visible in shared status; resource ordering is not a strict FIFO queue. Separate
clones/machines do not share locks and need an explicit common device owner.
All tasks using this checkout must follow the helper protocol; it does not
intercept arbitrary Git/Xcode commands or reconfigure already-running chats.
Keep task branches/worktrees until their integrated work and evidence are safe.

Protocol self-tests use temporary Git repositories, fake CI and simulated
resource holders; they never install or drive the iPad:

    python3 -I scripts/ipados/test_task_protocol.py
