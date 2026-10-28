# Accepted upstream failures for the development baseline

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
The source baseline commit and fresh full-CI validation are still pending at this
document's creation; native compilation and all 186 Foundation cases pass. See
[HANDOFF.md](HANDOFF.md) for the current result and SHA.

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
