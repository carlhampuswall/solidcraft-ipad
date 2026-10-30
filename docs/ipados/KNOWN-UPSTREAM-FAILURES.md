# Accepted upstream failures for the development baseline

## User extension — 0.27.1 development build, 2026-10-30

The user explicitly extended the exception for the **same three failures** to
the reviewed SolidCraft **0.27.1** iPad development candidate and authorized its
local integration into `develop`, installation and testing on the physical iPad.
The reviewed application/code tree is
`b10a8ee3e52200e2d4b63d1aaa391f6fc32c7406`, preserving the completed pen-selection
fix at `8544368f7282cb350bdfb07f6828f0919aa3445b` and upstream release
`aa1fe28ddff42cac2990a312765a1a6c50d97dfe`. This section is the only documentation
overlay on that reviewed tree for the initial integration.

The actual full `cargo +1.95.0 xtask ci` run remains **failed (exit 1)**:
**10,318 passed, three failed, 69 ignored**, with **86/87 test and side jobs
passing**. The three failure signatures exactly match the earlier preserved
baseline: the counterbored-ring measurement, robustness seed 36 and robustness
seed 71, listed below. Their tests, tolerances, local-prism implementation and
goldens are byte-identical to both the release and the earlier integration base.
No test is disabled, no tolerance widened and no golden data blessed.

The current full-gate log is retained in this task's
`target/upstream-verification/full-ci.log`, SHA-256
`6dd968dfd9ceef50b28527e2a6f32b926fda420feff5d180d15d8ad901c5e0ea`.
`target/upstream-verification/review.json` records the candidate, exact signatures
and other passing validation. Native adapter tests (206), the Foundation harness
(692 cases), strict Clippy, the unsigned iPad build and dependency guard pass.
Three preserved format-40 files round-trip through format-44 `.sc` and
`.solidcraft` containers; web offline/update, format-fold and seven actual Mac
GPU workflows also pass. Physical-device results are recorded separately for
the integrated commit; Mac GPU and synthetic input are not physical Pencil
verification.

This is a **local development-baseline exception**, not green CI or release
qualification. Seed 36 still produces invalid geometry. It permits no new or
changed failure, test suppression, unrelated kernel repair, push or publication
to `main`. Any application-code change beyond the reviewed tree requires fresh
affected validation and a new review of this bounded exception. The original
failed full-CI result must remain in the task record alongside the explicit
exception and any passing documentation-overlay checks.

## User exception — 2026-10-28

After the earlier blocked checkpoint, the user resumed Step 8.5 and explicitly
asked to leave the three already reported upstream engine failures unchanged,
record them and check again later. This permits a **development baseline with
these three known failures**. It does not make full CI green, accept additional
failures, certify release readiness or authorize kernel repairs in consolidation.

All three tests remain enabled. Their expectations, tolerances and golden data
remain unchanged. Run the required full gate and report its actual exit/result;
do not skip or suppress the tests to obtain success. Any new failed test, changed
failure signature or other unmet mandatory check blocks approval pending review.
Source baseline B is `c22acd12702cf3a405df01009b8d8662b3d46365`.

The completed full `cargo +1.95.0 xtask ci` run exits **1**: **85/86 top-level checks pass**; the engine library has **2631 passed, 3 failed, 6 ignored**. Only the three unchanged failures in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) remain under the user exception. All other required checks passed. **CI is not green.**

Completed full-gate log: `/private/tmp/solidcraft-step85-accepted-ci.log`. SHA-256: `c38a8f810f4f554f93fb47468631d48300f4cab250ffe0b8ba81394190e61526`.

Native compilation and all 186 Foundation cases pass. See HANDOFF for coverage distinctions and finalized policy P instructions; this exception does not launch tasks automatically.

## Exact accepted observations

The completed earlier full run, `/tmp/solidcraft-step85-ci-resumed.log`, had
2,631 engine tests pass, these three fail and six ignored. The later repaired
rerun passed its other 85 top-level checks, but its unchanged engine process was
explicitly interrupted after reporting these failures. Neither run is green CI.

| Enabled test | Expected → actual, or analytic comparison |
| --- | --- |
| `cmd::features::prism_local_tests::a_ring_cut_round_a_counterbored_hole_in_a_chamfered_disc` | With chamfer, removed volume **148.27973047857813 mm³** versus analytic **148.44025288211773 mm³**; existing relative tolerance **1e-3**. |
| `tests::robustness_goldens::seed_36` | Area **10408.825748 → 9964.083286**; edges **60 → 45**; faces **26 → 21**; volume **55729.313103 → 54852.503099**; outcome **ok → failed**; error becomes `lumps: shell boundaries intersect or touch; cavity ownership`. |
| `tests::robustness_goldens::seed_71` | Area **3980.574721 → 3980.581677**; edges **52 → 50**; faces **20 → 19**. |

Seed 36 produces invalid geometry; recording a platform result does not repair
it. The two other discrepancies also remain unresolved. The exception permits
continued development with these known limitations, not a claim that affected
geometry is correct.

The preserved older binary
`target/ci/deps/solidcraft_engine-7e865c9fa32174f9` reproduces all three identical
results. Logs are `/tmp/solidcraft-step85-prism-before-consolidation.log` and
`/tmp/solidcraft-step85-seeds-before-consolidation.log`; binary hashes and the
source-provenance limitation are in
`target/step8.5-verification/engine-binary-provenance.json`. Its timestamp does
not prove an exact source commit. This evidence supports a pre-existing platform
problem, but does not establish that every consolidation change is correct.

## Upstream evidence

These statuses were observed on 2026-10-28; recheck them at the next trigger.
No upstream source change was imported for this exception.

- [Issue #6944 — Engine regression validation on macOS](https://github.com/tangency-labs/solidcraft/issues/6944)
  reports all three exact tests on native macOS arm64, including the matching
  default ring measurement. It was open and assigned/in progress.
- [Issue #2669 — seed 71 on macOS arm64](https://github.com/tangency-labs/solidcraft/issues/2669)
  reports the same 52→50 edges and 20→19 faces on earlier upstream revisions;
  it was open.
- [Issue #3583 — seed 36 platform failure](https://github.com/tangency-labs/solidcraft/issues/3583)
  reports lost topology and cavity-ownership failure. Local
  [robustness documentation](../robustness.md) also records Windows/macOS failure
  and distinguishes repeatability from correct geometry. The issue was open.
- [PR #6946](https://github.com/tangency-labs/solidcraft/pull/6946) was closed
  unmerged in favor of [PR #6859](https://github.com/tangency-labs/solidcraft/pull/6859).
  The latter was open/unmerged, contained 34 files including unrelated desktop
  changes and did not have successful required checks; its
  [CI run](https://github.com/tangency-labs/solidcraft/actions/runs/40547533585)
  reported `action_required`. No narrow approved merged correction was available
  at inspection. Do not import that mixed change merely to alter these results.

Detailed read-only triage remains in
`target/step8.5-verification/upstream-blockers.md`.

## Required follow-up

The integration owner must re-evaluate this exception after a relevant reviewed
upstream correction, after engine/kernel/toolchain changes that may affect these
results, and before release qualification. Recheck upstream status, review the
small relevant delta, and run all three exact tests with the same workspace
feature graph and unchanged expectations:

```sh
cargo +1.95.0 test --locked --workspace --profile ci cmd::features::prism_local_tests::a_ring_cut_round_a_counterbored_hole_in_a_chamfered_disc -- --exact --nocapture
cargo +1.95.0 test --locked --workspace --profile ci tests::robustness_goldens::seed_36 -- --exact --nocapture
cargo +1.95.0 test --locked --workspace --profile ci tests::robustness_goldens::seed_71 -- --exact --nocapture
```

Then run affected integration checks and the complete `cargo +1.95.0 xtask ci`
on the actual candidate, preserving exit status and logs. Review geometry
correctness separately from any proposed platform-reference policy. Never bless
failures, widen tolerances, disable tests or copy unrelated upstream work to make
the report green. Remove an exception only with supporting successful evidence;
new failure cases require explicit review. No automatic monitor or scheduled
follow-up was created; these are manual integration/release checklist triggers.
