# Shared-directory development and iPad testing

The user authorized a return to one shared directory with no project build lock
on 2026-10-31, after the running tasks finished and merged. The view-menu icons
(`07c7a664`), two-finger rotation (`4f74b1ab`) and plane appearance (`6aaa4a67`)
are in local `develop`; their task-specific testing waivers remain recorded.
The later [device verification](HANDOFF.md) on 2026-10-31 covers those three
changes on candidate `85bde6a4`; physical finger/Pencil input remains unverified.
This replaces the previous task-worktree/integration/build-slot workflow.

## Work in the primary checkout

Use `/Users/chw/Documents/ChatGPT/SolidCraft - iPad` on `develop` for new tasks.
Inspect branch and status first. Local `develop` is authoritative; preserve
commits ahead of `origin/develop`. Do not create a branch or worktree for each
task, and do not switch branches while another task is editing or checking.

Tasks may work concurrently on independent files. Inspect existing edits and
agree ownership before overlapping changes, including shared interfaces. If
another task owns the same file, wait for that edit or coordinate a handoff;
do not overwrite its changes. Never auto-stash, reset, clean or broadly stage
another task's work. A dirty checkout alone does not block independent work.

The user authorizes local commits of each task's own reviewed changes after
relevant checks pass. Stage explicit owned paths (or reviewed owned hunks when
a file is shared), inspect the staged diff, and ensure it matches the source
that was checked. Do not use blanket `git add .` or `git commit -a`. Coordinate
staging/committing so another task's staged work cannot enter your commit.
A local commit on `develop` delivers the task; there is no per-task merge or
second integration gate. Report the commit and actual verification coverage.

Pushes, public PRs, merges into `main`, upstream integrations, history rewrites
and discarding another task's work need separate authorization. The permanent
rules in [UPSTREAM.md](UPSTREAM.md) and scoped AGENTS files remain in force.

## Check a finished batch, not every edit

**Engine tests require an explicit user request.** Do not run the engine suite,
including indirectly through `cargo xtask ci`, workspace-wide tests or a helper's
full-check option, unless the user explicitly asks for it for the current task.
The user reaffirmed this restriction on 2026-10-31. A shared-crate change alone
does not authorize engine tests. Use focused checks for the affected code;
session/bridge tests, GPU tests, linting, wasm checks and unsigned native builds
remain available. Report omitted full-CI coverage accurately. This restriction
takes precedence over broader CI instructions in this repository and its skills.

Implement and review a coherent set of changes first. Run the smallest relevant
checks once when that batch is ready to commit or hand off. Do not run tests,
build the app, or install on the iPad after every small edit. A new commit alone
is not a reason to repeat passing checks on identical source.

- Documentation-only changes need diff/link/content review; they do not need
  engine tests, an app build or device testing. Changed agent/skill instructions
  additionally use `cargo +1.95.0 xtask skills`.
- Changed development tooling uses its relevant tests. Protocol changes use
  `python3 -I scripts/ipados/test_task_protocol.py`; this needs no app build.
- Native UI batches use the unsigned device-target build and deterministic
  workspace harness. Adapter batches additionally use session/bridge tests,
  affected Clippy and Rust formatting. Add meaningful focused regressions where
  needed; do not add tests that merely mirror a trivial edit.
- Shared crates, desktop/web code, dependencies/workspace configuration and
  unfamiliar areas use the relevant compatibility checks at the completed-batch
  checkpoint. Full CI is still required for upstream/main/release qualification,
  but running its engine suite requires the explicit user request described above.

After a failure, fix it and rerun the failed checks and anything affected by the
fix. After later source changes, rerun only checks whose inputs or behavior were
affected; do not repeat unrelated suites. Freeze the source and dependencies
used by a running check, and never switch branches during it. Concurrent edits
can invalidate results: inspect the changes and recheck affected areas before
claiming success. Do not relabel partial coverage or an explicit user waiver as
passing tests. Task-specific waivers apply only to those tasks.

The helper is optional convenience for a completed batch, not a background
watcher or a mandatory step after every change:

    python3 -I scripts/ipados/task_protocol.py check --paths \
      scripts/ipados/task_protocol.py scripts/ipados/test_task_protocol.py

Without `--paths`, it examines all uncommitted and untracked nonignored files;
prefer explicit owned files when other tasks have edits. Use `--base <commit>`
for an already committed batch. `--check "command arguments"` adds a relevant
check without a shell. Direct focused commands are also valid.

Successful helper results are reused for identical working files, check plans
and environment, including across a commit with unchanged files. Changed inputs
invalidate reuse; a failed forced rerun invalidates its previous success. Use
`--force` after changes to installed tools or external/ignored test inputs, or
when investigating flaky behavior. Explicit `--full` always runs the full gate.
The conservative whole-checkout fingerprint can reject a check after an
unrelated edit; review that edit and rerun only the affected direct commands
rather than blindly repeating the entire batch. Receipts live in the ignored
Git common directory and contain check results, not environment secrets.

## Build directly

Run Cargo, the native build script and other build commands directly. There is
no project build lock, reservation, queue or integration lock. Independent
checks and builds may run concurrently. Cargo/Git/Xcode retain their own normal
locking; never remove their locks or `Cargo.lock` to bypass contention.

Reuse the primary checkout's `target/` and native derived data. Check CPU, memory
and disk capacity before heavy work and avoid redundant simultaneous builds.
Commands writing the same native products need coordination to avoid competing
writes; other independent work can continue. A Rust build plus Xcode packaging
is one pipeline: avoid replacing its Rust library midway through packaging.
Use reasonable per-process job limits when needed; there is no global build
slot. Preserve existing build outputs and test evidence unless cleanup is
separately authorized.

## Reserve the physical iPad only when testing is relevant

Physical iPad testing is a relevant acceptance checkpoint for device-dependent
behavior, not a ritual after every edit. Honor explicit task-specific opt-outs.
Prepare the signed build directly before reserving the device; a reservation
covers the complete install, launch, test and capture session:

    python3 -I scripts/ipados/task_protocol.py ipad feature-name --wait 60 \
      --candidate <prepared-commit> --device "$SOLIDCRAFT_DEVICE" \
      --evidence target/feature-name/ipad-session.txt -- \
      sh target/feature-name/test-ipad-candidate.sh

The shared checkout must be clean on the prepared commit. The task-owned script
must install that build, verify its candidate, run acceptance scenarios, capture
evidence and write the named fresh nonempty evidence file. The helper exports
`SOLIDCRAFT_CANDIDATE`, `SOLIDCRAFT_TASK` and `SOLIDCRAFT_DEVICE`. Compilation,
deterministic tests, synthetic input and physical Pencil observations are
separate coverage levels; report each honestly. A receipt records the tester's
result and is not automatic proof of hardware behavior.

For interactive testing use `--hold` instead of `--` and a command. Wait for
`IPAD RESERVED`, keep that process alive through the session and evidence
capture, then send `pass` or `fail` followed by a newline. EOF, interruption,
changed source/candidate or missing evidence cannot record a passing result.
Do not install, launch, terminate, drive or capture the app outside a reservation.
Do not launch simulators without an explicit user request.

    python3 -I scripts/ipados/task_protocol.py status

Status reports only the iPad reservation. The OS-backed lock is authoritative;
owner JSON is diagnostic. Direct child commands inherit the reservation until
they exit, even if the wrapper dies. Never detach device jobs or unlink lock
files. Waits are bounded to 0–60 seconds; continue independent work on contention.

## Preserved history

Existing task branches, worktrees, records, lock files and evidence are retained.
They are historical and do not require new tasks to use the old protocol. The
helper rejects retired `start`, `adopt`, `build`, `integrate`, `finish` and `note`
commands with instructions to use this shared workflow. Never delete old work
or rewrite another task's completion record to make a status display look clean.
Protocol self-tests use temporary repositories, fake build commands and device
reservations; they do not run the engine or access a physical iPad.
