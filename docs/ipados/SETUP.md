# iPadOS development setup

This fork prepares SolidCraft for a professional native iPadOS application while
keeping future upstream updates practical. Step 1 preparation is complete;
Step 2's native host and adapter are described in [ARCHITECTURE.md](ARCHITECTURE.md),
with current build instructions in [BUILDING.md](BUILDING.md) and verification in
[HANDOFF.md](HANDOFF.md). The preparation observations below are historical.

## Repository and branches

- `origin`: <https://github.com/carlhampuswall/solidcraft-ipad.git>
- `upstream`: <https://github.com/tangency-labs/solidcraft.git>
- `main`: the fork's integration base; preserve its history.
- `feature/ipados`: iPadOS development, initially branched from the fork's current `main`.

If working from a new checkout, clone the existing fork, never create another fork:

```sh
git clone https://github.com/carlhampuswall/solidcraft-ipad.git
cd solidcraft-ipad
git remote add upstream https://github.com/tangency-labs/solidcraft.git
git fetch origin
git switch --track origin/feature/ipados
git remote -v
```

## Layout

| Location | Responsibility |
| --- | --- |
| `apps/solidcraft-ipados/` | Reserved native iPadOS host, Apple project, resources and packaging |
| `crates/ipados/` | Reserved namespace for future narrow Rust integration crates |
| `docs/ipados/` | Fork setup, permanent rules and upstream integration procedure |
| `crates/geom`, `crates/kernel`, `crates/sketch` | Existing shared geometry and sketch solver |
| `crates/doc` | Existing parametric feature timeline, evaluation and document model |
| `crates/engine` | Existing session and command registry; application integration boundary |
| `apps/solidcraft`, `apps/solidcraft-web`, `apps/solidcraft-cli` | Existing desktop, web and CLI applications |
| `.github/workflows/upstream-sync.yml` | Upstream detection and optional draft integration PR creation |

The root workspace includes `apps/*` and `crates/*`. The native host and adapter
namespace remain excluded; the actual `crates/ipados/session` and
`crates/ipados/bridge` packages are explicit members. Target gates keep Apple SDK
requirements at the native boundary.

## Prerequisites for implementation

1. Read root [AGENTS.md](../../AGENTS.md), [CONTRIBUTING.md](../../CONTRIBUTING.md),
   the [architecture](../architecture.md), and [UPSTREAM.md](UPSTREAM.md).
2. Use macOS with full Xcode and its iPhoneOS SDK selected. Verify with
   `xcode-select -p`, `xcodebuild -version` and
   `xcrun --sdk iphoneos --show-sdk-path`. A physical iPad and a signing team/profile
   will be needed for device deployment; distribution requires the appropriate
   Apple Developer membership. Do not run simulators unless explicitly requested.
3. Install stable Rust meeting the root `Cargo.toml` MSRV (currently **1.95**),
   including rustfmt and Clippy. Use an explicit toolchain if your global default
   is unrelated to this project:

   ```sh
   rustup toolchain install stable --component rustfmt --component clippy
   rustup run stable rustc --version
   rustup target add --toolchain stable aarch64-apple-ios
   ```

   The [Rust iOS/iPadOS target documentation](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html)
   describes SDK discovery. Installing a target does not prove the whole dependency
   graph supports iPadOS; validate the smallest adapter first during implementation.
4. Keep Git and, for workflow operation, authenticated GitHub CLI available.
   Web compatibility checks also need `wasm32-unknown-unknown` and the existing
   Trunk tooling described in CONTRIBUTING.md. No JavaScript tooling is added;
   use pnpm if future JavaScript/TypeScript tooling is needed.

Before implementation, record the iPadOS deployment target, host UI technology,
Rust boundary, ownership/error handling, rendering integration and signing setup.
Upstream currently requires Rust-only application code and wasm compatibility.
Any proposal for a non-Rust native host must explicitly scope that exception to
the iPad host and leave the shared Rust rules intact. This preparation selects no
UI framework or FFI technology.

For lightweight preparation checks, use `cargo metadata --no-deps --format-version 1
--offline`, workflow linting and `git diff --check`; no full workspace build is
needed to validate these reserved directories. Implementation and upstream merges
must follow the validation gates in [UPSTREAM.md](UPSTREAM.md#validation-and-review).

## Environment observed during preparation (2026-10-27)

- Xcode 27.0 beta and the iPhoneOS 27.0 SDK are present.
- The default Rust toolchain is `esp` (Rust 1.90 nightly); installed stable is 1.93.
  Both are below the repository's Rust 1.95 minimum. Upgrade/select a suitable
  stable toolchain before compiling; global toolchains were not changed here.
- `aarch64-apple-ios` is installed for the existing stable toolchain. Install it
  for the updated toolchain as needed. Signing and physical-device deployment
  have not been configured or tested.
- Actions is enabled on the fork, but GitHub Actions PR creation is currently
  disabled. See [workflow activation](UPSTREAM.md#workflow-activation-and-credentials)
  for remaining repository setup. Inherited CI also uses Blacksmith runners;
  runner access for this fork has not been verified.
