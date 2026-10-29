# Task completion and local integration

This is the current workflow for new SolidCraft iPadOS tasks. It records the
user's 2026-10-29 request that completed tasks merge their progress. The older
B/P workstream instructions in [PARALLEL-DEVELOPMENT.md](PARALLEL-DEVELOPMENT.md)
remain historical provenance. Permanent engineering rules in
[UPSTREAM.md](UPSTREAM.md) and applicable validation gates still apply.

## Authorization and ownership

The user authorizes local commits of a task's own reviewed changes and local
integration into `develop` as part of finishing that task. This is the explicit
exception to the general instruction not to commit without being asked. It
does not authorize pushes, publishing PRs, merging into `main`, upstream
integration, history rewrites or discarding another task's work. Later user
instructions can narrow this authorization.

Each task owns delivery through integration, even when another chat performs
the merge. Assign one integration owner for the batch; with no batch owner,
the task owns its integration turn. Only one owner may update `develop` at a
time. A handoff to an integration queue is progress, not completion.

## Starting work

- Inspect branch, status and registered worktrees. Record the approved full
  local `develop` SHA and its relationship to fetched `origin/develop`. Local
  integrated work may be ahead of the remote; never reset it to the remote.
- Use a separate worktree and `codex/` branch for independent implementation.
  Keep build output and Xcode derived data inside that worktree. A task requiring
  another task's uncommitted changes must wait for their integration, or be
  handled sequentially by the same implementation owner.
- Give closely coupled UI changes one implementation owner. Agree shared API
  boundaries before editing; other tasks can inspect or test independently.
- Do not switch branches or merge in a checkout while another chat edits it,
  runs checks there, or has unrelated staged/unstaged work there.

The shared checkout already contained changes from several tasks when this
policy was written. Preserve and attribute those changes before consolidation;
never turn them into a task commit with a blanket `git add .`, reset or stash.
This policy does not establish ownership of those existing changes.

## Completion gate

1. Review the task diff and stage only its owned changes. Run relevant tests,
   formatting, Rust Clippy where applicable, and the required
   `cargo +1.95.0 xtask ci` gate before committing. Keep source files unchanged
   during verification. Apply existing user-approved testing exceptions only
   within their original scope and report them explicitly; this policy grants
   no new exception or permission to skip failing checks.
2. Commit the verified task changes locally. Record the starting SHA, task tip,
   owned files, checks and evidence. Review dependencies and the complete diff
   against the current `develop` before selecting the integration order.
3. Acquire exclusive integration ownership. Use an atomic lock in the Git common
   directory, resolved with `git rev-parse --git-common-dir`, so separate
   worktrees share it. An atomic `mkdir` of `solidcraft-integration.lock` under
   that directory may implement the claim. Record chat/task identity, process,
   checkout and acquisition time inside it. Hold ownership through candidate
   validation and the branch update; release only your own lock. A timestamp
   alone never justifies removing another owner's lock; verify its owner has
   stopped before recovery.
4. In a clean integration worktree, prepare the combined candidate from the
   recorded `develop` tip and task tip. Review conflicts and preserve both
   tasks' intended behavior; never resolve with blanket `ours`/`theirs`. Run
   affected checks on the combined candidate and the required full gate before
   any integration commit. Device testing, when required, uses the queue below.
5. Integrate with an ordinary local merge, preserving task ancestry; a verified
   fast-forward is sufficient when possible. Ensure `develop` has not moved
   since candidate validation. If it moved, rebuild and check the new combined
   candidate before updating it. Never force-update an occupied branch. Verify
   the task tip is an ancestor of the resulting `develop` with
   `git merge-base --is-ancestor`, and record the integrated SHA and check results.
6. Report completion only after integration and required verification succeed.
   If a dependency, conflict or required check blocks this, preserve the task
   branch and report **implemented; integration pending** with the exact blocker.
   Never claim a merge or device result that was not verified. Preserve source
   branches/worktrees until their work and needed evidence are confirmed safe.

## Physical iPad testing and communication

Use one device-testing owner and a shared queue for the batch. A request names
the exact candidate SHA, scenarios and required evidence. Installation, launch,
testing and evidence capture are one exclusive device session. Identify the
installed build in the results and release ownership before the next session.
If source changes afterward, repeat affected checks on the new candidate.
Combined testing of related UI changes is preferred when it covers each task's
acceptance criteria. Build, deterministic harness, synthetic device input and
physical Pencil observations must be reported separately. No simulators without
an explicit user request.

Keep routine status in the batch's shared record: task/chat, worktree, branch,
base/tip, dependencies, integration state, checks and device queue position.
Use chat messages only for a blocker, contract change, required handoff or
completion, within the user's authorization to message those chats. Polling and
repeated ownership broadcasts are unnecessary. Existing chats must receive this
policy through an authorized handoff or read it on their next turn; changing
these files does not interrupt or reconfigure a running chat.
