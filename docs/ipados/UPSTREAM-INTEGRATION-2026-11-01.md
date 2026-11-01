# Upstream integration — 2026-11-01

The requested update integrates 494 upstream commits into local `develop`, from
SolidCraft 0.27.1 to the 0.29.0 development sources. It preserves upstream
ancestry and the existing iPad host, adapters and shared-crate patches.

| Reference | SHA |
| --- | --- |
| Local development parent | `0d69b3f53b194dea79e533fc9d5ff9fa9078e6c3` |
| Previously integrated upstream ancestor | `8f1b4562758fd801e9fa95a1479fa2afb6914810` |
| Fetched upstream main | `bc4d35b0977d2f2c15df4297b91239e02b10bddd` |
| Unchanged fork main / origin main | `0accea759d36127154b99478601b4ba8ff71c899` |

This is a local development merge. No branch was switched, no remote was pushed,
and no PR or release approval was created. The ordinary clean-checkout/main PR
promotion in [UPSTREAM.md](UPSTREAM.md) remains outstanding. The existing dirty
native history-playback work was disjoint from the incoming changes; it was
archived before the merge and remains outside this integration's staged files.
Recovery files and verification logs are in the ignored
`target/upstream-integration-20261101/` directory.

## Merge review

Six files required manual conflict resolution:

- `engine/cmd/joints.rs` takes upstream's typed joint-edit contract, side
  validation and least-turn reseating. These supersede the fork's earlier
  equivalent support for replacing joint origins.
- `engine/lib.rs` exports both upstream's incremental `OpenSteps` API and the
  native adapter's stateless `navigate_camera` boundary.
- `engine/view.rs` keeps internal face-boundary sketches hidden and uses
  upstream's feature-placement visibility lookup.
- `gpu/capture.rs` keeps prepared model/guide/highlight capture and upstream's
  per-picture draw statistics within the same device lock. An integration
  regression checks that a native capture leaves no geometry or guide state in
  the next ordinary picture. Native proof callers consume per-picture statistics
  instead of upstream's removed `last_stats` API.
- `ui-egui/dialogs.rs` retains upstream's expanded stored-input reference helper
  and the fork's Clippy-compatible predicate.
- `ui-egui/sketch_dims.rs` retains construction-line dimensions and the
  Clippy-compatible rounded-rectangle predicate.

The native browser now reads upstream's typed browser entries through
`BrowserGroup::item_keys`, retains nested parent rows, and reflects inherited
visibility for empty and populated nested groups. A session regression exercises
the native rows and the real engine visibility command. The native GPU frame
uses the new `clouds` field and explicitly retains its existing lighting mode
with a disabled sky-fill field. The app and both adapter lockfile entries now
report 0.29.0.

Upstream's expanded Save naming exposed an adapter regression: a file-based
document rename advanced the native geometry revision and invalidated numeric
Apply. Saves now refresh cached browser labels without advancing that revision;
the existing ABI autosave regression also checks the new name and cached root
label. The engine still performs the save, rename and recovery-file validation.

Five incoming Clippy predicates in document projection/text and desktop inputs
were simplified for Rust 1.95's strict lint gate. No tolerances or expected
geometry were changed.

Upstream adds `sys-locale` 0.3.2 for desktop language selection, vendors its
`accesskit_macos` fix, and changes tempfile's existing getrandom dependency edge.
The licence and attribution gates cover the resulting dependency and asset
inventory. No additional integration dependency or copied CAD implementation
was introduced.

Upstream's released v45/v46 design migrations and pending changes are retained
alongside the fork's drawn-boundary and internal-profile pending files. Generated
catalogues, feature documentation and licence text come from upstream; none was
hand-edited for this merge.

## Verification

Checks use Rust 1.95.0 and the `ci` profile. Native compilation uses the selected
Xcode 27 / iPhoneOS SDK, a generic arm64 iOS destination, and disabled signing.
The native harness/build include the existing unstaged history-playback files;
those files are not part of the merge commit. Their hashes remained stable
through these checks.

| Check | Result |
| --- | --- |
| Rust formatting and diff whitespace | PASS |
| Assets, icons, licences, lists, skills, layers, UI containers | PASS |
| Workspace all-target Clippy, `--no-deps -- -D warnings` | PASS; affected test changes additionally rechecked |
| Document library | PASS: 520, one existing ignored |
| Sketch library | PASS: 323, three existing ignored |
| GPU library | PASS: 164, three existing ignored |
| GPU capture integration | PASS: 16, including native capture cleanup |
| iPad session library | PASS: 261, no ignored tests |
| iPad bridge library | PASS: 35, no ignored tests |
| Desktop UI library | Broad run: 4017 passed, 19 existing ignored, one replay failure; repaired parameter-dialog module: all 32 pass |
| Native Foundation/icon harness | PASS: 692 adaptive cases, 52 native tool mappings |
| iPad dependency guard | PASS: 13 shared crates / 181 resolved CAD packages |
| wasm compatibility | PASS: 14 crates; four inherited target-specific warnings |
| Web offline/update harness | PASS in Chrome with its stand-in app; not a complete web UI runtime build |
| Unsigned iPad application | PASS: Rust arm64 archive and Swift/UIKit compile/link |
| Mac GPU proofs | PASS: five appearance/grid stages, projected-profile selection/extrusion, assemblies and native save/reopen |

The projected-profile proof retains the 200 mm² partition over its visible
800 mm² source and produces a 2000 mm³ extrusion. The assembly proof retains
four components, hinge/rigid/slider joints, changed limits, analytic centroids,
5056 mm³ volume, preview cancellation and a native-file round trip. Actual dark
appearance and projected-profile extrusion captures were visually inspected.
These are Mac GPU/synthetic input checks, not physical iPad input evidence.

The initial broader regression run exposed the Save naming changes described
above: the ABI Apply regression required the adapter fix, and the reference-data
round-trip assertion now expects the destination-based document name while
comparing every other document field exactly. The full session and bridge
libraries pass after those corrections. Original failed logs remain available;
they are superseded only for the explicitly repaired checks.

The broad desktop UI run passed 4017 tests with 19 existing ignored tests and
one failed replay. Its long-expression test sent physical Ctrl+End, which the
locked egui version ignores on macOS; the typed suffix consequently landed in
the middle of the expression. The replay now sends explicit Command+Down and
checks the exact draft before Enter. Production text editing, the expected
expression and layout assertions are unchanged. All 32 parameter-dialog tests
pass after this test-only correction; unaffected desktop tests were not repeated.

The engine test suite and complete `cargo xtask ci` are excluded by the explicit
local policy in [TASK-COORDINATION.md](TASK-COORDINATION.md). Historical failures
in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) were not rechecked;
their previous exception is not evidence about the new candidate. Full upstream
CI, release qualification and physical iPad input/file-provider verification
remain outstanding. No simulator was launched or device installation performed.
