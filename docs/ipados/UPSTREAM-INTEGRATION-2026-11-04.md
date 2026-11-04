# Upstream integration — 2026-11-04

This update brings local `develop` from the 0.29.0 development sources to
SolidCraft **0.40.0-rc.2**, retaining the upstream ancestry and native iPad host.
The fetched snapshot contains 915 previously unintegrated upstream commits.

| Reference | SHA |
| --- | --- |
| Local development parent | `e81cf415b2bb64b774cc67c4e04e29ed8bd6c99f` |
| Previously integrated upstream ancestor | `bc4d35b0977d2f2c15df4297b91239e02b10bddd` |
| Fetched upstream main | `6488a540c9c48aed8714b863789fc6b2f9644faf` |
| solidcore kernel pin | `8ddce901671f1cebd9214a31d21895d725e886c8` |
| solidcore-record pin | `f5e8e9a29e0c9b7cf6c4040a5bc8878382b3c62f` |

This is a local development integration. Fork `main`, remote branches, signing
settings and the physical iPad are unchanged. Promotion through the main-branch
review/full-CI procedure in [UPSTREAM.md](UPSTREAM.md) is separate.
Logs and captures are kept in ignored `target/upstream-integration-20261104/`.

## Review and adaptation

Upstream replaces truck with the pure-Rust solidcore kernel and moves plain
geometry data into solidcore-record. The iPad adapter follows those existing
boundaries: it imports the shared record types and geometry traits, propagates
fallible world-state/scene/bounds queries, handles the added engine errors, and
preserves closed-body/hatch metadata in preview and instanced rendering.
The C ABI remains version 3. The native bundle's numeric marketing version is
0.40.0; the Rust workspace and adapter packages identify 0.40.0-rc.2.

Manual conflicts preserve both sides' intended behavior:

- `component.capture_position` keeps the fork's optional occurrence scope while
  respecting upstream's exclusion of display-only exploded placements.
- Multi-plane face extrusion, hidden internal profiles and drawn-boundary
  references remain. Extrusion inference uses upstream's new model-state API.
- The stateless native camera export coexists with upstream's camera-form API.
- Reference guides retain their draw order, with upstream's hidden edges drawn
  before visible strokes. Independent joint regressions remain in their
  existing test file.
- Upstream's browser reveal helper replaces the old local predicate.
- Retired truck modules and their lint-only patches are removed as upstream
  requires. The prior tangent plane/cylinder rail chamfer extension was part of
  that backend; solidcore rejects smooth tangent joins, so that narrow extension
  is unavailable after this update. The previous analytic regression source is
  preserved in the ignored evidence directory for future kernel work.

The exact upstream solidcore pin fails Rust 1.95 compilation at
`blend/blend_corner/mixed.rs` with E0308/E0277. Its generic pair helper needs an
explicit array-to-slice coercion before `nth_mut`. `vendor/solidcore/` contains
the pinned crate, licence and NOTICE, with that one marked source change and
standalone manifest metadata. All 511 source files were compared byte-for-byte
with the pin: only that coercion differs. No geometry implementation or kernel
revision is changed. See [vendor/README.md](../../vendor/README.md) and the
pending attribution row for provenance and removal conditions.

Incoming Rust predicates in STEP, SMLib conversion/sweeps, kernel validation,
document evaluation, engine panels/sketching, desktop panels and the CLI are
simplified for Rust 1.95's strict Clippy gate. Their finite-number, range and
branch behavior is retained. The existing native GPU capture regression now
unwraps the fallible camera/scene queries in test code.

The native surface handles now anchor an extrusion at the selected face point
and find an edge's halfway point by polyline length, keeping them independent
of the new tessellator's sample density. Cylinder-selection tests retain the
four independent rims and two independent walls, using solidcore's one closed
edge per circle and one face per cylinder. The sloped ring union now builds
without truck's retry warning; its analytic volume, history and round trip
assertions remain. No geometric tolerance or golden is changed.

Upstream's released document migrations, pending format changes, generated
catalogues and licence text are retained. The fork's two pending document
changes remain pending. Generated files were not hand-edited. The merged agent
router and test skill were shortened to meet the existing size gates while
retaining the shared-directory policy and explicit engine-test restriction.

## Verification

Checks use Rust 1.95.0 and the `ci` profile. Results below cover this development
candidate only; compilation and synthetic input do not establish physical iPad
behavior.

| Check | Result |
| --- | --- |
| All adapter targets, tests and GPU proof tools compile | PASS |
| Native Foundation workspace harness | PASS: 692 adaptive cases |
| Assets, icons, licences, lists, skills, layers, UI containers, mesh decisions | PASS |
| Pending document/drawing migration harness | PASS |
| Web offline/update browser harness | PASS; stand-in app, not a complete web UI build |
| iPad dependency boundary | PASS: 17 shared crates / 148 resolved CAD packages |
| iPad session library | 260 PASS, one recorded tangent-chamfer failure; no ignored tests |
| iPad bridge library | PASS: 35, no ignored tests |
| Document library | 563 PASS, two upstream known failures, one existing ignored |
| Geometry / sketch / renderer / I/O / profiler libraries | PASS: 351 / 332 / 299 / 207 / 8; eight existing ignored in total |
| GPU library and capture integration | PASS: 168 library tests (three existing ignored), all 16 capture tests |
| Desktop UI library | 4474 PASS, one upstream known variable-fillet failure, 19 existing ignored |
| Strict workspace all-target Clippy | PASS, `--no-deps -- -D warnings` |
| wasm compatibility | PASS: 16 crates; six inherited target-specific warnings |
| Unsigned native application | PASS: arm64 Rust archive and generic-device Xcode build; bundle version 0.40.0 |
| GPU/runtime proofs | PASS: appearance, projected profiles, assemblies and multi-face extrusion on the Mac GPU |
| Saved 0.29 assembly migration | PASS: four bodies, 5056 mm³, no timeline errors or warnings |

The GPU proofs check appearance changes, projected-profile selection and
extrusion, assembly joint limits/preview cancellation/save/reopen, and
multi-face new/join/intersect/cut operations. The appearance, extruded-profile
and cancelled-assembly captures were inspected visually. These are native
renderer proofs on the Mac GPU, not physical-iPad captures.

The fork delta passes Git's whitespace check with original third-party CRLF
sources preserved. The complete merge retains two existing upstream whitespace
diagnostics in `docs/kernel-cases.md` and
`tests/kernel-cases/fillets/KNOWN_FAILING`; those files match upstream exactly.

The ten-library Cargo batch exits **101**: **6697 passed, four failed, 31 ignored**.
An explicit comparison with `tests/KNOWN_FAILING_TESTS` finds exactly the three
upstream failures (two document tap-marking cost tests and the desktop variable
fillet round trip) plus the recorded native tangent chamfer, with no unexpected
failures and no now-passing entries. `test-grade.json` in the evidence directory
records that comparison. This is not a green full-CI result. The original
failure logs remain, including the backend-level chamfer refusal experiment;
upstream's normal smooth-edge guard is retained in the final source.

The engine suite and complete `cargo xtask ci` are excluded by the explicit
local policy in [TASK-COORDINATION.md](TASK-COORDINATION.md). Historical truck
failure exceptions are not validation of solidcore. Full upstream CI, release
qualification and physical iPad input/file-provider verification remain outside
this local check. No simulator is launched. The original native tangent-chamfer
test stays enabled and unchanged, with its solidcore refusal recorded in
`tests/KNOWN_FAILING_TESTS` under the upstream test policy. A nonzero test result
is reported as such; known failures are compared explicitly, never counted as
passing tests.
