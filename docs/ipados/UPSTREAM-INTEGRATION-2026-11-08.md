# Upstream integration — 2026-11-08

This update brings local `develop` from SolidCraft **0.40.0-rc.2** to
**0.40.0-rc.3**, preserving all 26 local commits ahead of `origin/develop` and
upstream ancestry. The fetched snapshot contains 369 new upstream commits.

| Reference | SHA |
| --- | --- |
| Local development parent | `a74ff9b3f8e7af540f1cbc521679def082424ef4` |
| Previously integrated upstream ancestor | `6488a540c9c48aed8714b863789fc6b2f9644faf` |
| Fetched upstream main | `ae1d85e5d4e32183258b3d301fb26b8687f4b969` |
| solidcore and solidcore-record pin | `a445bdaac60a980b0e9d1dba3ee28eb7ada44f50` |

This is a local development integration. No push, public PR, main-branch update,
signing change, simulator launch or physical iPad session is performed. Promotion
through the main-branch review/full-CI procedure in [UPSTREAM.md](UPSTREAM.md)
remains separate. Logs are in ignored `target/upstream-integration-20261108/`.

## Review and adaptation

Upstream updates the kernel, egui/eframe to 0.37 and open-format cadmpeg codecs
to 0.6.7. It adds rigid-body dynamics, typed command contracts, rendering and
placement fixes, and document migrations. Released v51 changes and the fork's
pending document changes are retained; generated files come directly from
upstream.

The ten manual conflicts are resolved individually:

- Capture Position keeps upstream's checked input/output contract and the
  fork's optional occurrence scope. The contract now accepts that scope,
  including an empty list to capture none, and preserves handler error wording.
  The existing adapter assembly regression exercises strict validation,
  malformed-input atomicity, unrelated pending moves and Undo/Redo.
- Extrude help retains multiple noncoplanar face support alongside upstream's
  clarified two-sided distance semantics.
- Desktop panel allocation and availability-based tab presentation use
  upstream's new implementation. Angle-plane controls retain the new direction
  option, and the formula-edit regression retains its pre-Enter assertion.
- The agent router retains the iPad exceptions, shared-checkout workflow and
  explicit engine-test restriction alongside the updated upstream guidance.
- Dependency, vendor documentation and known-failure lists retain current
  upstream changes and the existing native tangent-chamfer entry.

The new unmodified solidcore pin still fails Rust 1.95 with E0308/E0277 in
`blend/blend_corner/mixed.rs`. The existing temporary vendor patch is refreshed
to the new pin. All 605 source files match it byte-for-byte except the same
marked array-to-slice coercion. Manifest metadata includes upstream's new serde
dependency and optional profiler feature. Licences and NOTICE remain intact;
provenance is recorded in the pending attribution row. No geometry algorithm is
changed locally.

Rust 1.95's strict Clippy gate also requires four equivalent predicate
simplifications: embedded-texture validation, panel weight validation, desktop
panel resizing and render-light identity checks. Evaluation order and finite
number checks are preserved. Five calls in the existing GPU capture regressions
now pass the helper's new optional prepared-model argument explicitly; their
geometry and pixel assertions remain unchanged.

The native renderer supplies the newly required `preview_bounds` and `moving`
fields as `None`: its previews already use the adapter's prepared model scene.
The native C ABI remains version 3; numeric bundle version remains 0.40.0.

Upstream can now preserve unknown fields/features when opening newer documents.
The native adapter still validates evaluated snapshots strictly, so complete
native support for unknown-feature projects is not claimed by this update.

## Verification

Checks use Rust 1.95.0 and the `ci` profile where applicable. Final adapter,
desktop UI and GPU test executables are the Cargo artifacts recorded in
`final-host-tests.jsonl`; their checks run those exact binaries with source fixed.

| Check | Result |
| --- | --- |
| Rust formatting and merge whitespace | PASS; original third-party CRLF preserved |
| Assets, icons, licences, lists, skills, layers, UI containers, mesh decisions | PASS |
| Pending design/drawing migration harness | PASS |
| Strict workspace all-target Clippy | PASS, `--no-deps -- -D warnings` |
| Native Foundation workspace harness | PASS: 820 adaptive cases and icon checks |
| iPad dependency boundary | PASS: 18 shared crates / 148 resolved CAD packages |
| I/O embedded-texture regression | PASS: exact blob/provenance round trips and malformed metadata refusals |
| iPad bridge library | PASS: 40; no ignored tests |
| iPad session library | 291 PASS, one existing tangent-chamfer failure; no ignored tests |
| Focused desktop UI regressions | PASS: seven, no ignored tests |
| GPU capture regressions | PASS: four; actual Metal rendering, no adapter skips |
| Web offline/update browser harness | PASS; production update files with stand-in app, not complete CAD web runtime |
| wasm compatibility | PASS: 17 crates; 12 target-specific UI warnings |
| Unsigned native application | PASS: arm64 Rust archive and Xcode 27.0 / iPhoneOS 27.0 build, deployment target iPadOS 17 |
| Mac GPU profile/geometry proof | PASS: selected 200 mm² region extrudes to 2000 mm³; selected/extruded captures inspected |

The seven desktop regressions cover formula editing, angle-plane preview,
panel admission and availability fallback, fixed Bottom History height, and
light identity through numeric history and unrelated source replacement.
The GPU regressions cover moving layers and material opacity. The three profile
captures remain in the ignored evidence directory; they are Mac GPU output, not
UIKit screenshots or physical Pencil verification.

The final adapter run has **331 passes and one failure**, with the session
binary returning **101**. `adapter-test-grade-final.json` compares the observed
failure with the native entries in `tests/KNOWN_FAILING_TESTS`: no unexpected
failures and no now-passing entries. The tangent plane/cylinder chamfer still
reaches the smooth-edge refusal ("there is nothing to round") at the new pin.
Its test remains enabled with unchanged geometry and assertions. This is not
a green complete test suite.

Sandbox restrictions initially prevented the Foundation file-coordination
fixture, icon review server and Metal test context. Their permitted retries
passed; the original restricted logs are retained separately. Interrupted
pre-cleanup build attempts are not counted as successful validation.

The engine suite and complete `cargo xtask ci` are excluded by the explicit
local policy in [TASK-COORDINATION.md](TASK-COORDINATION.md). Compiling engine
code is not running its tests. Full upstream CI, release qualification, physical
iPad input and Files-provider verification remain outside this local check.
