# Building the native foundation

Use the existing fork on `feature/ipados`. Run from the repository root.
No simulator launch or device signing is part of the unsigned check.

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

The example uses real commands, rebuilds/tessellates the document and draws it
with the shared GPU capture/viewport pipelines. It requires an accessible GPU;
no CPU fallback is substituted for this proof.

Affected Clippy (`--no-deps` for the two adapter packages),
`cargo +1.95.0 fmt --all -- --check`, `cargo +1.95.0 xtask ci`,
and applicable web-offline/desktop checks remain required upstream gates. A native
build does not waive failures; see HANDOFF for results actually obtained.

## Signing and runtime validation

For installation, supply your own bundle ID/team and provisioning profile in
local Xcode settings, select a registered physical iPad, and enable its developer
mode as required. No team, credential, profile or signing secret is checked in.
The unsigned `.app` cannot establish successful launch or input behavior.
At inspection a physical iPad and development identities existed, but no installed
profile matched `com.solidcraft.ipados` or a wildcard application identifier.
No profile/account/identity was created or changed.

Coverage levels are separate: source/configuration prepared; Rust iOS target
compiled; native application linked; application launched; physical iPad verified.
Current measured coverage and blockers are in [HANDOFF.md](HANDOFF.md).
