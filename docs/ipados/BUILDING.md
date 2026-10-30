# Building the native iPadOS viewport

Use the existing fork's `develop` branch, or an isolated worktree at a recorded
approved `origin/develop` SHA. Run from that checkout root. The older B/P task
policy in [PARALLEL-DEVELOPMENT.md](PARALLEL-DEVELOPMENT.md) remains historical.
Current combined results and exclusions are in [DEVELOP.md](DEVELOP.md).
No simulator launch or device signing is part of the unsigned check. The
consolidation request excludes the engine suite and complete `cargo xtask ci`;
the full-gate examples below are for a later explicit full-test request.

## Prerequisites

- macOS with full Xcode, an iPhoneOS SDK and Swift 5 language mode support.
  Deployment target is iPadOS 17.0. Observed: Xcode 27.0 beta build 27A5228h,
  selected at `/Applications/Xcode-beta.app/Contents/Developer`, iPhoneOS 27.0 SDK.
  CI uses the full Xcode already selected on the hosted macOS runner and records
  its version. No global `xcode-select` change is needed.
  The workflow uses GitHub's standard `macos-15` label, listed in the
  [hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
- Rust **1.95.0**, the root manifest's MSRV, with rustfmt, Clippy,
  `aarch64-apple-ios` and `wasm32-unknown-unknown`. Always use explicit `+1.95.0`.
  The global `esp` default and existing stable/nightly toolchains are preserved.
- Python 3.11+ for repository validation tooling. No runtime Python/JS dependency.
- Adequate disk for native and cross-target artifacts; this checkout initially had
  about 16 GiB free. Full upstream CI needs substantially more than the adapter.

```sh
rustup toolchain install 1.95.0 --profile minimal --component rustfmt --component clippy \
  --target aarch64-apple-ios --target wasm32-unknown-unknown
rustup run 1.95.0 rustc --version
xcode-select -p
xcodebuild -version
xcodebuild -showsdks
xcrun --sdk iphoneos --show-sdk-path
```

For an alternate Xcode use `DEVELOPER_DIR=/path/to/Xcode.app/Contents/Developer`
for that invocation. Do not change accounts, identities or global defaults.

## Repeatable checks

```sh
cargo +1.95.0 test --locked --profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib
cargo +1.95.0 metadata --locked --format-version 1 --filter-platform aarch64-apple-ios \
  | python3 crates/ipados/check-dependencies.py
cargo +1.95.0 build --locked --profile ci --target aarch64-apple-ios -p solidcraft-ipados-bridge
sh apps/solidcraft-ipados/scripts/build.sh
```

The script sets the SDK and deployment target per process, builds Rust, then
builds `SolidCraft.xcodeproj` scheme `SolidCraft` for `generic/platform=iOS` with
signing disabled. Output: `target/ipados-xcode/Build/Products/Debug-iphoneos/SolidCraft.app`.
Cargo library: `target/aarch64-apple-ios/ci/libsolidcraft_ipados_bridge.a`.
An Xcode-only build requires that Rust library to exist; use the script first.
The simulator platform is declared, but no simulator Rust slice is built by this
device script; simulator execution is outside this task.

Host-GPU proof (macOS; does not launch the iPad app):

```sh
cargo +1.95.0 run --locked --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-ipados-render-proof.png
```

For canvas appearance evidence, append `--appearance` to that command. It writes
`-light.png`, `-dark.png`, `-grid-hidden.png`, `-grid-restored.png` and
`-restored.png`, checks the dark background pixels, and verifies that toggling
the grid and returning to Light restore their original images. This uses a
representative resolved native colour on the Mac GPU. On a physical iPad, also
check Appearance > Dark, Light and System, including a system appearance change
while the workspace is open; the canvas should follow the surrounding controls.
Check **Views > Visibility** for grid, origin point, axes and plane toggles, and
relaunch to confirm that the grid and reference preferences persist.

The example uses real commands, rebuilds/tessellates the document and draws it
with the shared GPU capture/viewport pipelines. It requires an accessible GPU;
no CPU fallback is substituted for this proof.

For Pencil selection evidence, append `--edge-selection`. It compares two taps
in the previous single-selection mode with the default Multiple mode, captures
two selected faces and two selected edges, then uses both edges in one Fillet and
verifies the analytic resulting volume. The `-before.png`, `-faces-selected.png`,
`-selected.png` and `-filleted.png` captures use the macOS GPU and synthetic Pencil
events; physical Pencil input still requires an iPad check. On an iPad, also turn
off **Tools → Multiple**, relaunch and verify that single selection is restored.

`--selection-taps` captures selection, empty-space clearing and reselection with
small Pencil lift motion through the real adapter and macOS GPU. Debug
`--selection-taps-check` runs face → empty space → face → same-face toggle through
the production native input mailbox on a physical iPad. It writes
`selection-taps-check.json` and four `selection-taps-<stage>.png` UIKit captures
in Documents, using a disposable fixture without loading or autosaving a library
project. Each capture follows a matching presented frame. The receipt includes
fresh timestamps and an optional `SolidCraftCandidate` Info.plist stamp.
Both checks use synthetic Pencil samples; actual Pencil contact remains separate.

For split-cylinder selection evidence, use `--cylinder-selection` for walls or
`--cylinder-edge-selection` for circular rims. Both capture independent outer
and inner selections as `-outer.png` and `-inner.png`, verifying that each tap
selects both native kernel pieces. These are macOS GPU captures with synthetic
Pencil events. On an iPad, check hover and tap on either half of each rim,
toggle it off from the other half, and confirm the other rims stay separate.

Affected Clippy (`--no-deps` for the two adapter packages),
`cargo +1.95.0 fmt --all -- --check`, `cargo +1.95.0 xtask ci`,
and applicable web-offline/desktop checks remain required upstream gates. A native
build does not waive failures; see HANDOFF for results actually obtained.

## Signing and runtime validation

For installation, supply your own bundle ID/team and provisioning profile in
local Xcode settings, select a registered physical iPad, and enable its developer
mode as required. No team, credential, profile or signing secret is checked in.
The unsigned `.app` cannot establish successful launch or input behavior.
Physical-device follow-up (2026-10-27): the app was signed, installed and launched
on the connected iPad Pro. The unrestricted signing check found an existing
Xcode-managed wildcard development profile covering the device; the earlier
restricted check had missed it. Automatic signing reused that profile and an
existing certificate without provisioning updates or account/identity changes.

After the Rust library is built, a signed local build can use command-line
overrides (supply your own existing team and device identifiers locally):

```sh
xcodebuild -project apps/solidcraft-ipados/SolidCraft.xcodeproj -scheme SolidCraft \
  -configuration Debug -sdk iphoneos -destination "id=$SOLIDCRAFT_DEVICE" \
  -derivedDataPath target/ipados-device CODE_SIGN_STYLE=Automatic \
  DEVELOPMENT_TEAM="$SOLIDCRAFT_DEVELOPMENT_TEAM" CODE_SIGNING_ALLOWED=YES \
  CODE_SIGNING_REQUIRED=YES build
xcrun devicectl device install app --device "$SOLIDCRAFT_DEVICE" \
  target/ipados-device/Build/Products/Debug-iphoneos/SolidCraft.app
xcrun devicectl device process launch --device "$SOLIDCRAFT_DEVICE" com.solidcraft.ipados
```

Xcode-managed profiles require automatic signing. Keep signing overrides local;
the checked-in build script remains the reproducible unsigned CI path.

Coverage levels are separate: source/configuration prepared; Rust iOS target
compiled; native application linked; application launched; physical iPad verified.
Current measured coverage and blockers are in [HANDOFF.md](HANDOFF.md).

## Step 3 development workflow

The existing build script and Xcode scheme are unchanged. Launch now generates
`SessionAdapter::fixture()` using real engine commands: a base, a separate post
and a visible reference sketch. Use Fit/Views/Projection, select a filter, tap
with Pencil or pointer, toggle Multiple and Clear. Finger drags orbit; two fingers
pan/pinch. Pointer secondary drag orbits, middle (or Shift-secondary) drag pans,
scroll zooms, and hover preselects. “Change post” edits the real post feature;
Undo and Redo use existing engine history. Status shows current geometry revision
and selected entity identities. No Files UI is added.

`render_proof` now captures the fixture and a taller post after `timeline.edit`:
`/tmp/solidcraft-ipados-render-proof.png` and
`/tmp/solidcraft-ipados-render-proof-changed.png`. These are macOS GPU pictures,
not iPad screenshots or performance measurements.

ABI 3 requires rebuilding both the Rust archive and the bundled Swift host.
Do not mix the Step 2 binary with the new header: engine, viewport and surface
responses now have separate copy functions to permit concurrent evaluation and
navigation. See `crates/ipados/bridge/include/solidcraft_ipados.h`.

For the opt-in Debug device sequence, launch with `--viewport-check` as shown in
[HANDOFF.md](HANDOFF.md). The native Debug configuration explicitly defines
`DEBUG`; release builds exclude that driver. It uses synthetic events and must
not be cited as actual Pencil verification. `view.project` returns view-local
points/drawable pixels for a known world point through the shared camera.

The native locale extraction phase intentionally runs on every Xcode build:
canonical and pending locale subfiles can change without their parent directory
mtime changing, so directory-only dependency analysis was insufficient.

## Step 4 input workflow

Select **Post radius** and drag the top-rim cross with Pencil. Fingers navigate;
Cancel interaction discards the preview. Undo/Redo use the real engine history.
The mapping, ownership and manual hardware checklist are in [INPUT.md](INPUT.md).

The same unsigned build script compiles all input and optional Pencil APIs against
an iPhoneOS SDK, targeting iPadOS 17; it neither signs nor launches an application.
The source-aware ABI and both bundled binaries must be rebuilt together.

For local engine/GPU evidence without an iPad:

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-step4-input.png --input
```

This writes before/preview/committed/undo/redo/cancelled PNGs and checks their
real engine geometry. It requires an accessible Mac GPU. Input is synthetic;
these are not iPad screenshots or physical Pencil results.

On a separately available, authorised physical iPad, a Debug launch argument
`--input-check` runs the synthetic edit/suspend/rebase/commit/history/cancel
sequence through the bridge and logs assertions. It does not run on normal
launch. Do not cite this driver as physical Pencil or palm-rejection verification.

## Step 5 workspace checks

```sh
sh apps/solidcraft-ipados/scripts/check-workspace.sh
cargo +1.95.0 test --locked --offline --profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib
cargo +1.95.0 clippy --locked --offline --profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --all-targets --no-deps -- -D warnings
sh apps/solidcraft-ipados/scripts/build.sh
```

The native check compiles/runs only Foundation/CoreGraphics layout/focus policy on
macOS; it does not boot a simulator or launch UIKit. On a separately authorised
physical iPad, Debug `--workspace-check` exercises the new bridge workflow with
synthetic calls and logs assertions. Actual interface taps, keyboard, Pencil,
VoiceOver and screenshot inspection remain the manual checklist in [UI.md](UI.md).
Debug `--tool-dismissal-check` uses a disposable launch fixture and production
joint forms to check Close, Properties toggle, browser replacement, empty-space
input and the legacy hide/reopen preference. It writes `tool-dismissal-check.json`
in the app's Documents directory. Input is synthetic; library projects are neither
loaded nor autosaved by this diagnostic.
Rebuild both Rust and Swift after adapter protocol changes; ABI 3 symbols remain
unchanged. Current observed results and the inherited full-CI blocker are in HANDOFF.

## Step 6 sketch checks

The same adapter tests, native layout/focus harness, unsigned build and wasm
checks now include sketch commands, constraints and dimensions. Exact coverage
and the current device availability are in [HANDOFF.md](HANDOFF.md); gesture and
command contracts are in [SKETCHING.md](SKETCHING.md).

DEBUG `--sketch-check` is an opt-in synthetic native-boundary smoke sequence. It
draws/dimensions/reopens a rectangle, changes its width, undoes/redoes and checks
native profile area. It is compiled but has not been run on a device. It neither
launches automatically nor substitutes for human UI/Pencil verification.

DEBUG `--sketch-toolbar-check` uses a disposable sketch and the production UIKit
controls. It checks icon titles and touch sizes after state refreshes, header
completion visibility while the rail scrolls, text/floating layout switches,
Light/Dark appearance, line/spline tools, and tapping **Finish sketch** to exit.
It writes `sketch-toolbar-check.json` and five `sketch-toolbar-<index>.png`
captures in Documents without opening or autosaving a library project.
Physical iPad verification on 2026-10-29 passed all five variants and actual
button completion; the captured UIKit screenshots were inspected in both
appearances. The native harness passed 550 adaptive cases and icon validation,
and unsigned/signed device builds passed. This scoped host change does not
claim a full Rust workspace CI run or physical Pencil verification.

For reproducible Mac GPU model/feedback evidence (no UIKit or device):

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-step6-sketch.png --sketch
```

This exercises actual synthetic Pencil rectangle creation, driving dimensions,
reopening, width preview/commit and undo/redo, checks native area and writes eight
PNG captures. The final three show dimension arrows before, during and after a
direct touch drag, with the measured length and grab offset verified. Native
dimension text and workspace chrome require UIKit;
they are not part of these offscreen model captures.

For direct sketch geometry movement in Select:

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-sketch-drag.png --sketch-drag
```

This captures `-before.png`, `-previous.png`, `-preview.png` and `-committed.png`.
It compares the saved handle preference's previous Select behaviour with direct
dragging, verifies a selected rectangle translates by (20, 15) mm while retaining
its constraints and closed profile, and checks that one Undo restores it. The
session regressions cover free points/edges, selected groups on a YZ plane,
partial/fixed constraints, taps and jitter, cancellation, returning to the start,
two-finger navigation and the previous preference setting. These are synthetic
input and macOS GPU checks; physical iPad gestures remain separate verification.


## Step 7 modelling checks

The existing adapter tests now include analytic modelling, native timeline edits,
reference repair, persistence and deterministic handle/preview races. Run the
same explicit Rust1.95 ci profile and unsigned build; no simulator is required.
The focused filter is `modelling_`. The existing native workspace harness also
checks production modelling response decoding and exact reference/expression
preservation. Final observed counts/gates are in HANDOFF.md.

Mac GPU model evidence (does not launch UIKit or a device):

```sh
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-step7-model.png --modelling
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-cli -- \
  eval /tmp/solidcraft-step7-model.solidcraft
```

The proof saves the actual native design for separate upstream CLI validation.
Use PATH=/opt/homebrew/bin:$PATH for xtask on this machine, retaining its
project-scoped Python3.11+ and Node tools; do not alter global toolchain defaults.

## Step 8 assembly checks

Use the same adapter/native checks above. Assembly acceptance lives in the area's
existing `session/src/tests.rs`; the name filter is `assembly_`. The bridge tests
also verify native source-mesh instance selection and bounded triangle-cache
reuse across changed occurrence transforms. For the two additive shared APIs:

```sh
cargo +1.95.0 test --locked --offline --profile ci -p solidcraft-engine --lib joints -- --test-threads=1
cargo +1.95.0 test --locked --offline --profile ci -p solidcraft-engine --lib occurrence -- --test-threads=1
cargo +1.95.0 check --locked --offline --profile ci -p solidcraft --all-targets
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- /tmp/solidcraft-step8-assembly.png --assemblies
cargo +1.95.0 run --locked --offline --profile ci -p solidcraft-cli -- \
  eval /tmp/solidcraft-step8-assembly.solidcraft
```

The GPU proof uses actual parametric components and native rigid/revolute/slider
commands, analytic world-pose checks, scratch previews/cancel and native
save/reopen. The deterministic adapter tests separately verify Undo/Redo. It
writes11 model/feedback captures for inspection. These are Mac
GPU images, not UIKit/device screenshots. The separately rebuilt shared CLI can
read the same native file; CLI validation is not desktop UI testing.

No simulator is required or authorised. Current physical-device verification,
exact counts/logs, shared compatibility changes and mandatory CI blockers are in
[HANDOFF.md](HANDOFF.md). [ASSEMBLIES.md](ASSEMBLIES.md) documents workflow and
reference/units conventions.

The opt-in DEBUG physical driver uses the production UIKit modelling form
callbacks and native session on the connected hardware:

```sh
xcrun devicectl device process launch --device "$SOLIDCRAFT_DEVICE" \
  --terminate-existing --console com.solidcraft.ipados --assembly-check --assembly-check-pause
# After each logged READY stage and actual capture:
xcrun devicectl device process signal --device "$SOLIDCRAFT_DEVICE" \
  --pid "$SOLIDCRAFT_PID" --signal SIGUSR1
xcrun devicectl device copy from --device "$SOLIDCRAFT_DEVICE" \
  --domain-type appDataContainer --domain-identifier com.solidcraft.ipados \
  --source Documents/step8-assembly-check.log --destination /tmp/solidcraft-step8-device-native.log
```

Without `--assembly-check-pause`, the same command/frame barriers advance
automatically. The flag replaces only the disposable launch fixture and saves a
unique Documents/Step8-Assembly-Runtime-UUID.solidcraft; normal launch never runs
the driver. Matching frame callbacks, native replies and watchdog failures drive
the sequence. It does not simulate physical Pencil contact. Human input must be
checked separately. The final observed physical build/launch, CLI evaluation of
the device-written file and beta CoreDevice screenshot limitation are documented
in HANDOFF; no screenshot is repaired to conceal a capture-service failure.
Adding `--assembly-pencil-check` opens the production hinge drive at its current
30-degree value after verification, evaluates without Apply and leaves both
panels closed with the ring visible for a human Pencil check. It adds no saved
CAD or input-simulation behavior.


## Step 8.5 verification environment

Use Rust `+1.95.0` explicitly. On this Mac, `/usr/bin/python3` is older than
the repository's Python 3.11 requirement; `/opt/homebrew/bin/python3` is available
and supports `tomllib`. Select it per command with
`PATH=/opt/homebrew/bin:$PATH`, without changing global Python/Rust/Xcode settings.
Use the equivalent installed Python 3.11+ path on other hosts.

The icon review and web offline checks bind localhost HTTP servers; a restricted
runner must grant those local capabilities. A sandbox permission failure is not
a test assertion failure, and neither counts as a pass.

```sh
PATH=/opt/homebrew/bin:$PATH CARGO_BUILD_JOBS=3 SOLIDCRAFT_TEST_JOBS=3 \
  RUST_TEST_THREADS=3 cargo +1.95.0 xtask ci
sh apps/solidcraft-ipados/scripts/check-workspace.sh
sh apps/solidcraft-ipados/scripts/build.sh
```

The initial full test build was deliberately stopped when free space fell below
1 GiB. No source/build files were cleaned. The user then freed space; recheck
`df -h .` before resuming. Preserve `target/step8.5-preservation/`, which contains
the pre-consolidation recovery archive and Git bundle. Separate task worktrees
must use separate target/derived-data directories; four simultaneous clean
native builds need substantially more capacity than one warm checkout.
