# Step 8.5 checkpoint and resumed decision

## Superseding user decision — 2026-10-28

The user resumed Step 8.5 and explicitly asked to leave the three already
reported upstream engine failures unchanged, document them and check again
later. The earlier blocked decision and recovery instructions below are retained
as a historical record; they are superseded only for those three known failures
by [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md).

This permits a development baseline with disclosed CI failures, not a green-CI
claim or a release certification. Tests remain enabled, expectations/tolerances
unchanged, and any new failed test or changed failure signature blocks approval.
The current combined candidate passes a fresh unsigned native build and all 186
Foundation cases (`/tmp/solidcraft-step85-resume-native-{build,check}.log`).
Source baseline B still needs the fresh full-CI result and a commit; the exact three known failures alone do not
block that commit. Recheck them after relevant upstream fixes, affected
engine/kernel/toolchain updates and before release. No automatic monitor exists.

The candidate also preserves subsequent user-requested two-finger double-tap
Undo / three-finger double-tap Redo from chat “Add multitouch undo and redo”
(`01a183dd-c2f4-74a1-a93d-024ef30a9e7c`). The default-on preference
`SolidCraft.workspace.historyGestures` restores prior behavior when disabled.
Its separate unsigned build and 186 existing layout cases passed; physical
gesture verification remains pending. The UI overhaul chat remained read-only.
The archives and counts below describe the earlier checkpoint, not this later
candidate. No Tasks A–D were launched. Use HANDOFF for fresh outcomes and exact
baseline/policy SHAs when available, without substituting the old Step 3 HEAD.

---

# Historical Step 8.5 recovery checkpoint — superseded as noted above

Recorded 2026-10-28; upstream statuses below were observed during this task.

**Blocked; Tasks A–D are not approved to start.** The user chose “Preserve
checkpoint; keep Tasks A–D blocked.” Preserve this consolidation candidate and
its evidence. Do not start new features or kernel repairs as part of this task.

**Full gate remains failed; the repaired rerun was explicitly interrupted.**
The first full run completed: engine library 2,631 passed, 3 failed, 6 ignored;
its other crash/catalog failures were subsequently repaired. In the repaired
rerun, 85 of 86 checks passed (83 binaries plus wasm/doc jobs), including all
2,932 UI tests and 87 desktop-app tests. The unchanged engine binary again
reported all three failed test names; its duplicate process was stopped with
SIGTERM after the user's checkpoint decision. The rerun exited 1, not success.
No test expectation or golden was changed. Logs are preserved under
`target/step8.5-verification/solidcraft-step85-ci-{resumed,final}.log`.
There are no remaining consolidation build/test processes to resume.

## Preserved state

- Repository: `https://github.com/carlhampuswall/solidcraft-ipad`; upstream:
  `https://github.com/tangency-labs/solidcraft`.
- Checkout: `/Users/chw/Documents/ChatGPT/SolidCraft - iPad`.
- Branch: `integration/ipados-foundation`; HEAD:
  `fc58c334d997e7514f96d6b44b753f272dad9e91`. This is the Step 3 ancestor,
  **not** a consolidated baseline. All Steps 1–8 were found locally; Steps 4–8
  and consolidation repairs remain in the working tree, 72 dirty paths before this checkpoint document was added.
- No consolidation commit, push, merge into main or future-task launch occurred.
  Earlier branches/worktrees remain intact. No physical iPad, simulator or
  signing operation was performed during Step 8.5.
- `docs/ipados/CONSOLIDATION-INVENTORY.md` records original SHAs/provenance;
  `INTERFACES.md`, `PARALLEL-DEVELOPMENT.md` and `NEXT-TASKS.md` describe the
  contracts and proposed ownership. The task prompts retain
  `BASELINE_PENDING_VERIFICATION`; do not substitute HEAD.
- The last reported account usage observation was **44% remaining**, with the
  user's **10% remaining** stopping threshold. This is a checkpoint observation,
  not a refreshed reading; check limits before costly continuation and honor
  that threshold.

The original 47 modified/untracked files, per-file SHA-256 manifest, tracked and
staged patches, refs/reflogs and verified complete `history.bundle` are retained
in `target/step8.5-preservation/`. The later frozen candidate is separately saved
in `target/step8.5-verification/`:

| Artifact | SHA-256 at checkpoint |
| --- | --- |
| `final-candidate-working-files.tar.gz` | `7c861f17f9558a1208abd8ce71fb2e188746e14894338af0e306f05fa0361c3f` |
| `final-gate-source-manifest.json` (per-source-file hashes) | `3162d92ecd5714e2e917521156a10817d1683a08e3f25fa38a4620bfe2417e8c` |

`final-candidate.patch`, copied build/test/GPU evidence and
`engine-binary-provenance.json` are beside them. These ignored local archives
are recovery evidence, not a remote backup. Before moving/deleting a checkout or
its build directory, copy them to a safe location and verify hashes. Inspect or
extract into a separate directory; never overwrite ongoing work or apply patches
blindly. The final checkpoint is saved as `docs/ipados/CHECKPOINT.md`; its additional documentation changes are archived separately in `blocked-checkpoint-working-files.tar.gz`. The frozen archive above remains unchanged.

## Known engine blockers

The earlier full run, `/tmp/solidcraft-step85-ci-resumed.log`, completed the
engine library with **2,631 passed, 3 failed, 6 ignored**; seed 81 eventually
passed. The three failures are unchanged expectations, not accepted new goldens:

| Exact test | Observed versus expected |
| --- | --- |
| `cmd::features::prism_local_tests::a_ring_cut_round_a_counterbored_hole_in_a_chamfered_disc` | Chamfered ring removed volume **148.27973047857813** versus analytic **148.44025288211773 mm³**, exceeding the existing relative tolerance `1e-3`. |
| `tests::robustness_goldens::seed_36` | Area **10408.825748 → 9964.083286**, edges **60 → 45**, faces **26 → 21**, volume **55729.313103 → 54852.503099**; outcome **ok → failed**, error `lumps: shell boundaries intersect or touch; cavity ownership`. This is invalid geometry, not merely platform-equivalent topology. |
| `tests::robustness_goldens::seed_71` | Area **3980.574721 → 3980.581677**, edges **52 → 50**, faces **20 → 19**. |

All three reproduced with the same results in the preserved older engine binary
`target/ci/deps/solidcraft_engine-7e865c9fa32174f9`. Logs:
`/tmp/solidcraft-step85-prism-before-consolidation.log` and
`/tmp/solidcraft-step85-seeds-before-consolidation.log`. Its SHA-256 and timestamp
are recorded in `engine-binary-provenance.json`; a timestamp does **not** establish
an exact source commit. These reproductions and upstream reports support a
pre-existing platform issue, but do not prove every consolidation change correct
or make the current gate pass.

Primary upstream evidence, read during this task:

- [Issue #6944](https://github.com/tangency-labs/solidcraft/issues/6944) reports all
  three tests on native macOS arm64, including the matching ring measurement.
- [Issue #2669](https://github.com/tangency-labs/solidcraft/issues/2669) reports
  seed 71's matching macOS topology discrepancy.
- [Issue #3583](https://github.com/tangency-labs/solidcraft/issues/3583) reports
  seed 36's topology loss and cavity-ownership failure; local
  `docs/robustness.md` also records failure on Windows and macOS.
- [PR #6946](https://github.com/tangency-labs/solidcraft/pull/6946) was closed
  unmerged. Its replacement [PR #6859](https://github.com/tangency-labs/solidcraft/pull/6859)
  was open/unmerged, contained 34 files including unrelated desktop changes,
  and lacked successful required checks at inspection. Its
  [CI run](https://github.com/tangency-labs/solidcraft/actions/runs/40547533585)
  reported `action_required`. No narrow approved merged correction was found.

Detailed triage is retained in
`target/step8.5-verification/upstream-blockers.md`. No failing expectations were
blessed, tolerances widened, tests disabled or unrelated upstream PR imported.
The duplicate engine run was explicitly interrupted as recorded above.

## Resume only through reviewed corrections

1. Read this checkpoint, HANDOFF, UPSTREAM and applicable AGENTS/skills. Inspect
   Git status, worktrees, preserved hashes and remaining account/disk capacity.
   Preserve the current tree and evidence; the prior gate and interrupted rerun
   have both ended. Do not treat their existing logs as a passing new candidate.
2. Recheck upstream issue/PR state. Review a narrow relevant correction and its
   compatibility impact before integration. Do not blindly merge the 34-file
   PR, waive seed 36's invalid geometry, bless failed results or expand this
   checkpoint into unapproved kernel development. If exact ancestral-source
   comparison is needed, use a separate checkout/build directory at the recorded
   SHA with the same Rust 1.95.0, lockfile, profile and feature graph.
3. After an approved relevant fix, run each failure explicitly from the reviewed
   candidate using the pinned toolchain and unchanged expectations:

   ```sh
   cargo +1.95.0 test --locked --workspace --profile ci cmd::features::prism_local_tests::a_ring_cut_round_a_counterbored_hole_in_a_chamfered_disc -- --exact --nocapture
   cargo +1.95.0 test --locked --workspace --profile ci tests::robustness_goldens::seed_36 -- --exact --nocapture
   cargo +1.95.0 test --locked --workspace --profile ci tests::robustness_goldens::seed_71 -- --exact --nocapture
   ```

4. Run affected integration tests and required native/desktop/web/wasm,
   formatting, Clippy, layering, licensing and compatibility checks described in
   BUILDING/HANDOFF. Then run the complete `cargo +1.95.0 xtask ci` on the final
   source without switching branches or editing beneath it. Retain exact logs;
   partial passes do not approve the baseline. No simulator launch is authorized.
5. Only after required validation passes, commit source baseline B. Finalize
   both task documents and HANDOFF with its exact SHA, then commit docs-only
   policy P after its required gate. Record both immutable SHAs as described in
   PARALLEL-DEVELOPMENT; a document cannot contain its own commit hash. Obtain
   launch approval. Push only when authorized, never to main. Until then keep
   Tasks A–D blocked and preserve all earlier work.
