# Step 2 handoff

Branch: `feature/ipados` in the existing `carlhampuswall/solidcraft-ipad` checkout.
Preparation/base commit: `0d548f65`. The Step 2 commit is the commit containing
this handoff (`git log -1 --format='%H %s'`); its exact SHA is reported with the
implementation handoff. No upstream merge, history reset or push is part of this
work. The initial working tree was clean.

## Start here

- Decision/patch ledger: [ARCHITECTURE.md](ARCHITECTURE.md).
- Exact build path: [BUILDING.md](BUILDING.md).
- Scope/Step 3 acceptance: [IMPLEMENTATION.md](IMPLEMENTATION.md).
- `crates/ipados/session/src/lib.rs`: safe `SessionAdapter`, real commands,
  structured errors, document/geometry/scene revisions and owned `RenderSnapshot`.
- `crates/ipados/bridge/include/solidcraft_ipados.h`: ABI 1 ownership and buffer
  contracts; `src/ffi.rs` is the only unsafe module. `src/renderer.rs` drives the
  existing shared viewport renderer, not a replacement renderer.
- `apps/solidcraft-ipados/SolidCraft/{Application,NativeSession,MetalCanvas}.swift`:
  lifecycle, worker/render lock and typed native input. Xcode target/scheme:
  `SolidCraft`; deployment iPadOS 17, iPad device family only.
- `.github/workflows/ipados-build.yml`: scoped PR/manual macOS build checks;
  no Blacksmith assumption. Workflow source is prepared; GitHub execution has
  not been dispatched or observed.

## Commands and observed results (2026-10-27)

All Cargo commands used `+1.95.0`; default Rust remains `esp`. Dependencies kept
their locked external versions. Local SDK: Xcode 27.0 beta / iPhoneOS 27.0.

| Command/check | Observed result |
| --- | --- |
| `rustup toolchain install 1.95.0 --profile minimal --component rustfmt --component clippy --target aarch64-apple-ios --target wasm32-unknown-unknown` | Installed successfully; global default unchanged. |
| `xcode-select -p`, `xcodebuild -version`, `xcodebuild -showsdks` | Selected `/Applications/Xcode-beta.app/Contents/Developer`; 27A5228h; device and simulator SDKs present. No settings changed. |
| `cargo test --locked --offline --profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib` | 8 adapter + 5 ABI tests pass. Real timeline rebuild, 12,000 mm³ body, engine tessellation, release/lifetime, bad commands/input, view revision and bounded copies. |
| `cargo build --locked --offline --profile ci --target aarch64-apple-ios -p solidcraft-ipados-bridge` | Rust native device library compiled. |
| `cargo check --locked --offline --profile ci --target wasm32-unknown-unknown -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib` | Both libraries pass; inherited wasm engine dead-code warnings remain. |
| `cargo clippy --locked --offline --profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --all-targets --no-deps -- -D warnings` | Pass for both new packages. |
| Same Clippy command without `--no-deps` | Fails in unchanged `crates/geom/src/fill_general.rs:166`, `clippy::nonminimal_bool`; platform lint success does not waive this failure. |
| `cargo metadata --locked --offline --format-version 1 --filter-platform aarch64-apple-ios` piped to `python3 crates/ipados/check-dependencies.py` | Pass: 13 shared crates / 177 resolved CAD packages; no platform/frontend dependency leaks. |
| `cargo fmt --all -- --check`, `git diff --check` | Pass. |
| `cargo xtask layers`, `skills`, `lists`, `ui-containers` | Pass (19 registered crates; scoped exceptions keep router/skill size limits). |
| `cargo check -p rfd --target aarch64-apple-ios --locked --offline` | Deliberate architecture spike fails: 12 missing native dialog backend implementations. Locked arboard iOS check passes but selects X11 backend. |
| `sh apps/solidcraft-ipados/scripts/build.sh` | PASS: latest Rust device library rebuilt and unsigned native `SolidCraft.app` linked, `BUILD SUCCEEDED`. First sandboxed Xcode attempt could not run nested `sandbox-exec`; reran outside the Codex sandbox with Xcode's own script sandbox retained. |
| `cargo run --locked --profile ci -p solidcraft-ipados-bridge --example render_proof -- /tmp/solidcraft-ipados-render-proof.png`, then direct execution of that binary outside the sandbox | Sandbox could not access a GPU. Host execution PASS: Apple M3 Pro Metal, 1 engine body, 12 triangles, 12,000 mm³, 12 drawn triangles. PNG visually inspected; it shows the real rectangular solid. This is macOS GPU evidence, not an iPad screenshot. |
| `PATH="/opt/homebrew/bin:$PATH" cargo xtask ci` | FAIL exit 101 in existing `crates/geom/src/fill_general.rs:166`, `nonminimal_bool`. Before that: fmt, 843 attributed assets, icon checks (17 tests), licences, lists, skills, layers and containers pass. Earlier attempts exposed Python 3.9 and sandbox localhost restrictions; corrected invocation uses Python 3.14 and runs outside the sandbox. |

Full workspace tests, complete workspace wasm, desktop smoke and web-offline
validation were not reached after the Clippy failure. Trunk is not installed.
This is **not** a passing complete repository gate or approval/merge-ready claim.
No checks were disabled or marked passing to hide that failure. The focused iPad
workflow lints the two new packages with `--no-deps`; it does not replace the
existing full upstream gate.

Build logs live under `/tmp/solidcraft-ipados-*`; they and generated artifacts
are not source-controlled. App output:
`target/ipados-xcode/Build/Products/Debug-iphoneos/SolidCraft.app`.
Evidence: `/tmp/solidcraft-ipados-render-proof.png`.

## Verification limits and next work

Source/configuration, Rust iOS compilation and native application linking are
proven. No iPad application launch, Pencil runtime test or simulator
launch has been performed. A physical iPad is connected and valid development
identities exist; none of 11 installed profiles matches `com.solidcraft.ipados`
or a wildcard app identifier. No signing/account/profile changes were made.

The current native app submits its only CAD operation before surface attachment.
Before Step 3 adds expensive commands while rendering, split engine evaluation
locking from synchronous render suspension. Preserve document/scene generations
and typed input, and use the shared `view.navigate` camera command/path. File
access/autosave/restoration and interactive selection/gestures remain unimplemented.
Do not expand this foundation into a second command system or camera controller.
