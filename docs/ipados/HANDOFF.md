# Step 3 handoff — interactive native CAD viewport

Continue in this existing checkout on `feature/ipados`. Base: Step 2
`dc96ee14`. Its three uncommitted physical-device documentation follow-ups were
preserved below. No reset, upstream merge, dependency upgrade, toolchain default
change or push was performed. The Step 3 commit is the commit containing this
handoff; `git log -1 --format='%H %s'` gives its identity.

## Start here

- [IMPLEMENTATION.md](IMPLEMENTATION.md): feature scope; [ARCHITECTURE.md](ARCHITECTURE.md):
  ownership, revision and upstream delta; [BUILDING.md](BUILDING.md): build/run.
- `crates/ipados/session/src/lib.rs`: `SessionAdapter::{execute,selection,fixture,proof,snapshot}`;
  the one mutable engine session and owned `Arc<SceneSnapshot>` publication data.
- `crates/ipados/session/src/viewport.rs`: `Viewport::{begin_update,install,fail_update,
  set_metrics,command,input,snapshot,selection,diagnostic}`; the single native
  display camera, generation gating and independent guide/highlight scenes.
- `crates/ipados/session/src/picking.rs`: `ViewportMetrics::{validate,pixel,pixels}`,
  `PickFilter`, `PickScene::pick`; shared `MeshIndex`, topology IDs and clipped
  world rays, screen-space tolerances and occlusion checks.
- `crates/ipados/session/src/input.rs`: typed `InputEvent`, `InputSource`,
  `InputPhase` and central basic contact routing. This is Step 4's extension point.
- `crates/ipados/bridge/src/ffi.rs`, `include/solidcraft_ipados.h`: ABI **2**.
  `sc_request`/`sc_response_*` are the serial engine lane; `sc_view_request`/
  `sc_view_response_*` the interactive lane; `sc_surface_*` and
  `sc_surface_response_*` the independently serialised GPU/lifecycle lane.
  Rebuild Swift and Rust together. `renderer.rs` retains model GPU uploads by
  document/geometry revision and uses separate guide/highlight slots.
- `apps/solidcraft-ipados/SolidCraft/{Application,MetalCanvas,NativeSession}.swift`:
  safe-area controls, typed UIKit events, engine/display workers and the render-only
  lifecycle gate. The Xcode project extracts native messages on every build so
  pending catalogue edits cannot leave stale message IDs in native controls.
  The host also explicitly follows UIWindowScene coordinate-space updates and
  refreshes point/pixel metrics when display scale changes at the same pixel size.

## Repeatable workflow

The fixture is generated through actual `solid.box`, `solid.cylinder`,
`sketch.create`, `sketch.rectangle.two_point` and `sketch.finish` commands. It is a
base, a separate raised post and a visible reference sketch. It is an ordinary
parametric document; the generic adapter supports other engine designs.

One finger orbits; two pan/pinch. Pencil/pointer taps select. Pointer hover
preselects; secondary drag orbits, middle or Shift-secondary drag pans, scroll
zooms. Fit, seven standard views, projection, selection filter, Multiple and
Clear are native controls. Diagnostic identity is body name plus face/edge index,
or the engine's vertex point. “Change post” executes `timeline.edit`; Undo/Redo
use existing history. All topology selection is conservatively cleared after
geometry/visibility changes. There is no persistent topology promise.

Debug builds accept `--viewport-check` for an opt-in device workflow: navigation,
standard views, body/face/edge/vertex selection, multi-select, clear, projection,
post edit, undo and redo. It uses **synthetic** Pencil-labelled samples through
the same input boundary; it does not establish hardware touch/Pencil behavior.
The driver asserts each selected entity type and logs its identity. Add
`--viewport-check-hold-face` to stop after a selected face for a stable screenshot.
`view.project` is the read-only adapter projection helper used to target known
world points in that workflow. Normal launch does not run it. For example:

```sh
xcrun devicectl device process launch --device "$SOLIDCRAFT_DEVICE" \
  --terminate-existing --console com.solidcraft.ipados --viewport-check
```

## Checks observed on 2026-10-27

All Cargo invocations use `+1.95.0`, `--locked`, `--profile ci`, and local/offline
dependencies. No external dependency versions changed. Xcode 27.0 beta/iPhoneOS
27.0; deployment target remains 17.0.

| Check | Result |
| --- | --- |
| `cargo test -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib` | PASS: 19 safe adapter + 9 ABI tests. Real engine geometry/edit/undo/redo/deletion; snapshot lifetime; world transforms; known topology picks; occlusion/hidden geometry; scale/offset/aspect/projection; empty/tiny/large/off-origin scenes; close orthographic zoom; stale generations/presented revisions; typed input, consumed UI, pan/pinch, hover/multi-select; malformed input; independent suspension/response lanes; malformed-command recovery keeps a valid viewport usable; smooth-sphere body feedback reuses model GPU blocks, and camera movement retains face highlight geometry. |
| Engine `cmd::view_navigation::tests::continuous_navigation_moves_the_camera_without_editing_the_design` | PASS: existing shared navigation test, 1 run, 2,619 filtered. |
| Shared GPU `capture::tests` | PASS: both existing opacity/face-colour conversion tests. |
| Affected adapter/bridge/GPU Clippy `--all-targets --no-deps -- -D warnings` | PASS. |
| Device dependency guard | PASS: 13 shared crates, 177 resolved CAD packages; no platform/frontend leaks. |
| Adapter, bridge and GPU `cargo check --target wasm32-unknown-unknown --lib` | PASS; three inherited engine dead-code warnings remain. |
| Swift `-typecheck -D DEBUG` | PASS against arm64 iOS 17/iPhoneOS 27 SDK. |
| `sh apps/solidcraft-ipados/scripts/build.sh` | PASS: Rust device archive and unsigned native app, `BUILD SUCCEEDED`. Xcode requires execution outside the outer Codex sandbox to run its own script sandbox. |
| Signed Debug device build | PASS with existing team/profile/certificate and local overrides; `codesign --verify --deep --strict` passes outside the outer sandbox. No provisioning updates, account or checked-in signing changes. |
| `cargo xtask web-offline-test` | PASS with existing Chrome after allowing its localhost server outside the outer sandbox. |
| `cargo xtask ci` | BLOCKED by inherited `crates/geom/src/fill_general.rs:166`, `clippy::nonminimal_bool`. Before it: fmt, assets, icons (17 tests), licences, lists, skills, layers and UI containers pass. A diagnostic run past that expression also found six inherited kernel boolean lints. Geometry/kernel files remain unchanged. Full workspace tests/wasm/desktop smoke are not claimed passing. |

Full CI remains a required upstream integration gate. A local review commit does
not waive this baseline failure and is not merge approval. No check was disabled,
no lint allow added and no toolchain/global setting changed to hide it.

## Evidence and runtime verification

- `render_proof` executes the real fixture, renders it, changes the post from
  25 to 40 mm with `timeline.edit`, and renders the new revision. Apple M3 Pro,
  shared Metal GPU, Rust `ci` profile: 988 → 1,108 triangles, both bodies drawn.
  It also picks and captures body, face, edge and vertex feedback using the
  native adapter's exact GPU block references. A separate selected smooth sphere
  proves visible body tint without model edges or copied feedback triangles. These are
  **macOS GPU images**, visually inspected, not device or frame-rate evidence.
  Paths: `/tmp/solidcraft-step3-viewport{,-changed,-body,-face,-edge,-vertex,-sphere}.png`.
  Physical selected-entity captures are `/tmp/solidcraft-step3-ipad-{body,face,edge,vertex}.png`;
  redo is `/tmp/solidcraft-step3-ipad-redo.png`. Face feedback and revision/identity
  readouts were visually inspected; complete manual hardware verification remains below.
- Physical device: iPad Pro 11-inch (3rd generation), using the existing pairing
  and development signing setup. Step 3 installs, launches and renders the real
  fixture on its native Metal layer. Labels were visually rechecked after fixing
  stale catalogue extraction. The asserted 23-stage synthetic workflow completed:
  body `Base`, face/edge index 5 and vertex `[-20,-15,10]`, multi-selection, clear,
  projection, edit, undo and redo; the final capture shows geometry revision 9.
  CoreDevice changed physical-device orientation to portrait during the run;
  the same four picks passed afterward, then the original landscapeLeft was restored.
  This is software-injected device coverage, not physical input hardware coverage.
  Captures during CoreDevice orientation changes can crop the client area, so
  mathematical resize/picking coverage is stronger than visual rotation coverage.
  The final host coordinate-space callback was subsequently compiled and installed;
  its relaunch was blocked because the iPad had become locked. The final scene
  layout, same-pixel-size display-scale and GPU body-tint/resource-reuse corrections
  are build-verified (and the GPU tint Mac-verified) only; they need an unlocked
  iPad recheck before hardware coverage can be claimed. The user explicitly
  deferred further iPad checks and authorised simulator or other verification;
  the final verification uses real-engine tests, Mac GPU captures and iOS builds.
  No simulator was launched. This does not block starting Step 4 from this commit.
  Final binaries are rebuilt, but no final locked-device launch is claimed.
- Private device screenshots and logs live under `/tmp/solidcraft-step3-*` and
  are not committed/published. Some captures contain unrelated device overlays.
  Build products remain ignored in `target/ipados-{xcode,device}/`.
- Actual Pencil taps/hover, physical simultaneous finger gestures, pointer
  hardware, manual multitasking, memory pressure and complete lifecycle stress
  coverage remain unverified. No simulator was booted or launched. No FPS or
  device timing claim is made.

## Step 4 extension points and limits

Extend `InputEvent`/`Contacts` in `input.rs` for simultaneous source arbitration,
palm rejection and cancellation/coalesced-event policy. UIKit source identity,
pressure/altitude, timestamp, modifier/button and last-presented geometry stamp
are retained. Pointer hover is wired; Pencil hover and double-tap need compatible
hardware and explicit Step 4 work. Keep navigation on the existing shared path.

Keep the engine and display workers independent. Evaluation/tessellation/index
construction run on the engine worker; no kernel preemption is available.
Generation invalidation rejects results, retaining the last valid scene and
reporting failure while current picking is disabled. Do not introduce another
session owner, Swift camera controller or topology identity scheme.

Shared source changes are the small stateless navigation export in
`crates/engine/src/{lib.rs,cmd/mod.rs,cmd/view_navigation.rs}` and the additive
`Capturer::rgba_viewport` verification seam in `crates/gpu/src/capture.rs`. Both
are Apple-independent; existing navigation/capture contracts retain their defaults.
The latter verifies native model-block feedback through the existing GPU/readback
pipeline. Check those small signatures when updating upstream. No geometry,
kernel, sketch, renderer pipeline, format, dependency or broad upstream merge. Section/analysis UI, mesh-only topology,
image/decal interaction, document browser, sketch editing and polished UI remain
outside this milestone.

---

## Preserved Step 2 handoff and physical-device follow-up

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
| `PATH="/opt/homebrew/bin:$PATH" cargo xtask ci` | FAIL (xtask exit 1; Clippy child exit 101) in existing `crates/geom/src/fill_general.rs:166`, `nonminimal_bool`. Before that: fmt, 843 attributed assets, icon checks (17 tests), licences, lists, skills, layers and containers pass. Earlier attempts exposed Python 3.9 and sandbox localhost restrictions; corrected invocation uses Python 3.14 and runs outside the sandbox. |

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

Source/configuration, Rust iOS compilation, native application linking and a
physical iPad launch/render are proven. Pencil interaction, rotation, memory
pressure and full foreground/background lifecycle tests remain unverified.
No simulator has been launched.

Physical-device follow-up (2026-10-27, user-authorized install):

- Built the unchanged application from `dc96ee14` using local automatic-signing
  overrides and an existing wildcard development profile/certificate. The earlier
  restricted profile check had missed the available profile. No profile, account,
  certificate, trust setting or checked-in signing configuration was changed.
- Signed Xcode build passed; `codesign --verify --deep --strict` passed outside
  the sandbox; `devicectl device install app` and `device process launch` passed.
- Physical iPad Pro 11-inch (3rd generation), iPadOS 27.2, Developer Mode enabled.
  Process remained running. Device screenshot visibly shows “Native engine and
  renderer ready” and the real command-generated solid on the shared Metal renderer.
- Signed app: `target/ipados-device/Build/Products/Debug-iphoneos/SolidCraft.app`.
  Local logs: `/tmp/solidcraft-ipados-signed-build.log`, install/launch/process JSON
  under `/tmp/solidcraft-ipados-*`. Screenshots remain private local evidence and
  are not committed or published; they also contain unrelated device overlays.

The current native app submits its only CAD operation before surface attachment.
Before Step 3 adds expensive commands while rendering, split engine evaluation
locking from synchronous render suspension. Preserve document/scene generations
and typed input, and use the shared `view.navigate` camera command/path. File
access/autosave/restoration and interactive selection/gestures remain unimplemented.
Do not expand this foundation into a second command system or camera controller.
