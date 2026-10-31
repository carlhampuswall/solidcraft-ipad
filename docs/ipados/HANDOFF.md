# Consolidated develop handoff

Steps 1–8, Step 8.5, the isolated UI workstream and the completed orbit, origin,
project-library and extrusion tasks are consolidated on `develop`.
[DEVELOP.md](DEVELOP.md) is the current branch/provenance, verification and
continuation report. `main` and all original source branches remain preserved.
The source and per-task results below are historical records; their older test
totals, single-save-slot descriptions and no-commit/no-push status do not describe
the final combined branch. The engine suite is excluded from this integration's
checks at the user's request; no full-CI success is claimed.


## Circular cut selection after sketch resizing — 2026-10-31

A synthetic 80 × 80 × 4 mm plate, face-attached sketch and radius-45 mm
circle reproduce the reported silent whole-body subtraction when the base sketch
shrinks to 40 × 40 mm. Projected face edges split the selected circle into two
graph regions with ambiguous curve lists; their saved list indices subsequently
refer to different regions. Capture now retains the complete drawn circle's
whole source boundary, and resolves its current graph pieces by that identity.
Partial selections stay partial. The pending format marker protects older readers;
previously saved index selections are not reinterpreted by guessing lost intent.

The regression failed before the fix with zero remaining volume and passes after
it with the analytic remaining volume (about 594.44 mm³), including history-marker
navigation and native save/reopen. Five document profile-reference tests, all 209
session tests and all 30 bridge tests pass. CLI before/after CPU captures and
measurements are in ignored `target/sketch-resize-investigation/`. A local repaired
copy of the affected saved design also rebuilds at both 40 and 80 mm without
feature warnings or errors. Formatting, affected Clippy with warnings denied,
pending-format checks, adapter/bridge WebAssembly checking and the unsigned iPad
build pass. WebAssembly retains the existing unused `Exported.bodies` warning.
The first Xcode attempt was blocked by the outer sandbox; its unchanged-source
retry completed outside that sandbox. The original on-device design was only
read; the app was not installed, launched or driven. Engine tests/full CI remain
excluded by the current task policy.

## Deferred-change device verification — 2026-10-31

Signed candidate `85bde6a4c1eed2441da473b420aa1ec48d2e73fd` was installed and
verified on the physical iPad Pro 11-inch (3rd generation), iPadOS 27.2. This
covers the previously deferred View menu icons, two-finger twist and compact
origin-plane appearance. The native acceptance report passes all eight stages;
all eight UIKit/Metal captures were retrieved and inspected. The original-plane
switch persisted both values. Injected finger contacts produced camera rolls
of +0.4 and −0.4 radians, and zero with twist disabled, while preserving geometry
and selection and clearing the contacts. The app was then relaunched normally
and its saved project was visibly restored.

Testing found and fixed a regression where compact styling also shrank custom
construction planes, hiding a tilted plane inside the fixture solid. Custom
construction planes now retain their original display/pick extent and colour;
XY/XZ/YZ retain the new compact appearance. All 195 session tests, 28 bridge
tests, affected Clippy, Rust formatting, 692 native adaptive cases, icon checks
and unsigned/signed builds passed. No full workspace CI or simulator was run.

Evidence is in ignored `target/deferred-changes/`: `ipad-session.json`,
`deferred-changes-check.json`, eight `deferred-*.png` images and `logs/`.
Earlier failed diagnostic attempts are preserved in `attempt1/` and `attempt2/`;
the driver was corrected for mounted-page counts and modal sequencing. Popovers
in this run used the saved Dark preference; the canvas was exercised in both
appearances. Physical finger/Pencil delivery and hands-on gestures remain
unverified; the touch sequence was injected through the native bridge on the
real device and awaited matching presented frames. Library projects were not
loaded or autosaved during the diagnostic. All preferences were restored.

---

# Pencil selection of finished sketch profiles — 2026-10-28

Finished, shown sketch regions are now selectable in the model view with Pencil
or pointer under Face/Auto. The selected region has a filled blue highlight and
boundary; Extrude receives its exact native profile identity without requiring a
dropdown choice. Same-sketch multiple selection and canvas reselection into an
open modelling form preserve entered dimensions. Explicit topology filters,
hidden geometry, profile holes, solid occlusion and stale-scene guards remain
covered. Profile detection and modelling semantics stay in the existing engine.

The two initial regressions failed before the fix (empty Pencil selection and
empty profile after reselection). All 124 current session and 28 bridge unit
tests pass, including five new profile regressions. Affected-crate Clippy with
`-D warnings`, the wasm adapter check, formatting and unsigned arm64 iPad build
pass. The wasm graph retains its existing
engine dead-code warnings. Test/build logs are `/tmp/solidcraft-profile-tests.log`
and `/tmp/solidcraft-profile-build.log`.

`render_proof --profiles` captured and checked actual shared-GPU output on macOS:
two finished regions, the right-hand 200 mm² region selected through synthetic
Pencil input, and only that region extruded 10 mm to 2,000 mm³. Evidence is in
`target/ipados-evidence/profile-selection/` (ignored build output). GPU capture
also exposed the highlight pass's unused glass lane; translucent profile fills
now use its alpha-blended triangle lane, covered by the bridge regression.
This is software input/rendering evidence, with no new physical Pencil/UIKit
verification, device installation or simulator launch. No commit or push was made.

# Project library and file exchange — 2026-10-28

Local implementation adds a separate native Projects browser with search,
create/open, duplicate and confirmed inactive-project deletion. Native containers
are stored in UUID directories; the previous single save slot is migrated without
removing its source. Committed changes autosave, switching saves first, and the
selected project restores on launch (with a preference for starter-model launch).

Files import accepts SolidCraft, STEP/STP, STL, OBJ and 3MF as a new project, or
inserts supported CAD/mesh geometry into the current design. Export offers STEP,
SolidCraft, STL, OBJ and 3MF via the system destination picker. External reads are
security-scoped/coordinated and staged with size limits. Candidate loading and
saving finish before document replacement. Autosave avoids viewport replacement;
exports run against a frozen container on a separate worker. Engine commands own
all parsing, serialization and CAD semantics. No shared crate changes, new
dependencies, design format, commit, push or device installation were made.

Validation: 117 session and 26 bridge tests pass, including native/STEP round
trips, duplicate renaming, failed replacement/history preservation, and saves that
retain numeric preview state. The macOS Foundation harness passes library name,
same-name identity, deletion-confinement and coordinated-staging checks alongside
330 layout cases and 41 icon checks. Affected Clippy (`-D warnings`), wasm adapter
checks, formatting, sorted pending catalogues, dependency layers, UI containers,
plist validation and the unsigned arm64 iPad build pass. The wasm dependency graph
retains its existing engine dead-code warnings. Logs are
`/tmp/solidcraft-project-{rust-tests,workspace,clippy,wasm,fmt,lists,layers,containers,build}.log`.
Xcode and file-coordinator checks needed an unrestricted runner; their restricted
attempts were capability failures and were not counted as passes.

No simulator was launched. This change has no new physical-device/UIKit/Files
provider runtime verification or screenshot evidence. The preceding installation
record below predates these project changes. Use the Projects checklist in UI.md
on a physical iPad before treating that coverage as complete. Full workspace CI
was not run for this uncommitted change.

# Origin visibility follow-up — 2026-10-28

The Part browser now has an expandable Origin references group with individual
eye controls for O, X/Y/Z and XY/XZ/YZ. The group menu and View menu offer Show
all, Hide all and Automatic during sketch creation. Automatic is the default;
explicit saved choices override it. Cancelling the chooser clears selections
that become hidden. No shared engine, CAD data or undo-format change was needed.

Validation: 117 session and 26 bridge tests, 330 native workspace cases, 41 icon
checks, Clippy with warnings denied, formatting, locale lists and the adapter's
wasm check pass. The unsigned iPad build passes. The real Mac Metal origin proof
captures default-hidden, automatic chooser visibility, individual hiding, manual
XY visibility and cancellation; representative captures were inspected. This
follow-up does not claim physical testing of the new browser controls or full CI.
Logs and captures use `/tmp/solidcraft-origin-browser-*`.

---

# One-finger rotation direction correction — 2026-10-28

The user reported inverted one-finger rotation in the combined installation.
Removed the extra negation of both drag axes from the object-centred input path.
The visible object now follows the finger; the fixed body-centre pivot and
screen-relative rotation through poles remain. The existing legacy-orbit
preference and wire keys remain compatible; its menu text now reads
"Rotate around object with one finger". The direction regression checks projected
object motion across both up axes, standard views, rolls, projections and scales.
It failed before the correction and passes after it. Existing pivot/framing,
gesture-transition, planar, pan and pointer regressions also pass.

Verification: 117 session and 26 bridge tests, affected-crate Clippy, formatting,
iPhoneOS Rust build and signed UIKit build pass. The existing Mac GPU orbit proof
reports zero object-centre drift and its captures were inspected. Strict signature
verification, installation, normal launch and the live physical iPad display pass.
Physical finger direction still requires the user's hardware check; the Mac GPU
driver uses synthetic finger samples. No engine suite, full CI or simulator ran.

To avoid including unfinished concurrent native project-browser edits, the signed
host was reconstructed from the previously installed source hashes at
`/private/tmp/solidcraft-orbit-host-i2rzk04i`, then linked with a private copy of the
corrected Rust archive. The shared source checkout retains the correction and all
other tasks' work. Nothing was committed or pushed by this correction.

Logs: `/tmp/solidcraft-orbit-direction-{before,tests,clippy,ios-build,gpu,signed-build}.log`.
The before log records the expected regression failure. Source hashes, host/app
paths and device results are retained as `orbit-*` files in
`target/latest-ipad-integration/`. This installation supersedes the one below.

---

# Combined local iPad installation — 2026-10-28

The user's build/install request combines the working origin-reference and
viewpoint-orbit changes with UI branch `codex/ipados-ui` through
`22d4308ee1838f7786b2531f45e4f410f7c32447`. The integration checkout remains on
`380b43b2047a1242c81dfec75142283a0b61bd76`; this combination is uncommitted.
No branch history was changed or pushed. UI overlap was reconciled to preserve
the full-window layout, visual plane picker, origin visibility, orbit preference
and existing compatibility preferences. Original UI-overlapping working files,
the prior patch and source hashes are preserved under
`target/latest-ipad-integration/`.

The combined tree passes 115 session and 25 bridge tests, 330 native layout cases,
41 native icon checks, formatting, asset attribution and the icon gate. Unsigned
and signed arm64 builds pass. The signed application passes strict code-signature
verification and was installed and normally launched on the physical iPad Pro
11-inch (3rd generation), iPadOS 27.2, using the existing development profile.
Device Hub's live display shows the floating workspace, actual shaded fixture,
origin references and full-window canvas. CoreDevice confirms the installed app
remains running. This is launch/display verification; physical Pencil gestures
were not exercised. The engine suite and full CI were not run.

Logs: `/tmp/solidcraft-latest-adapter-tests.log`,
`/tmp/solidcraft-latest-workspace-check.log`,
`/tmp/solidcraft-latest-native-build.log`,
`/tmp/solidcraft-latest-signed-build.log` and
`/tmp/solidcraft-latest-icon-check.log`. Installation, launch and running-process
results are in `target/latest-ipad-integration/{install,launch,process}.json`.

---

# UI overhaul workstream — 2026-10-28

Branch: `codex/ipados-ui`. Isolated checkout:
`/Users/chw/.codex/worktrees/ipados-ui/SolidCraft - iPad`.
Source baseline B: `c22acd12702cf3a405df01009b8d8662b3d46365`.
Finalized policy P: `380b43b2047a1242c81dfec75142283a0b61bd76`, verified as a
documentation-only descendant of B. P supersedes B's provisional planning
documents. This branch does not merge P or modify the integration checkout.
No push or merge is authorized/performed.

## Implemented presentation

The actual UIKit app now uses a floating document header, 52/56-point icon-first
rail, create/modify/pattern/assembly groups, immediate sketch tools and contextual
constraints/dimensions/construction. Right-side orientation/Fit/projection/
Properties and bottom Undo/Redo use existing commands. Selection/tool/solver/
snapping/error status is compact. Diagnostic tools and routine assembly reports
are outside the normal workspace. Browser/Properties start closed; unpinned
floating panels preserve the Metal frame and consume only visible bounds. An
optional pinned wide browser reserves space. Short numeric transactions use a
compact editor; native feature/assembly forms retain real Preview/Apply/Cancel.

Follow-up: remove the unused safe-area bands. The workspace now fills the window;
chrome is placed within safe bounds and the floating Metal canvas extends behind
the status/home indicators. Rendering and picking use that same enlarged native
surface. Pinned-browser reservations and keyboard resizing are preserved. The
text-layout compatibility preference keeps its original inset canvas policy.

Canonical SolidCraft icons are generated as Xcode vector assets using the
existing palette/alias resolver: 321 masters, 37 aliases and 481 discovered
command-icon mappings. A bounded image cache handles theme and density. No SVG,
review hash, command registry, native DTO or CAD state changed. The user approved
the resource-only Xcode project registration outside C's original ownership.

Changed files:

- `apps/solidcraft-ipados/SolidCraft/{WorkspaceController,WorkspaceComponents,WorkspaceLayout}.swift` (layout sections only in WorkspaceLayout)
- `apps/solidcraft-ipados/SolidCraft.xcodeproj/project.pbxproj` (resources only)
- `apps/solidcraft-ipados/scripts/generate-icons.py` (new build tool)
- `apps/solidcraft-ipados/scripts/{check-workspace.sh,workspace-check.swift}`
- `assets/locales/pending/ipados-ui-refinement.json`
- `docs/ipados/{UI,HANDOFF}.md`

Changes a default: yes — compact floating chrome, with the earlier text rail,
header and docking policy behind **Tools → Compact floating workspace** off
(`SolidCraft.workspace.compactChrome`). Dark native chrome defaults only when
there is no existing appearance preference; **Appearance → System** retains the
old policy. Rail-side and history gesture preferences remain unchanged.

## Verification and real evidence

- PASS: `sh apps/solidcraft-ipados/scripts/check-workspace.sh`: 330 adaptive
  layout/focus/decoder cases, plus 41 actual tool icon mappings, vector theme
  assets, retired-output removal and unchanged canonical artwork/review data.
- PASS: actual iPhoneOS arm64 Swift type checking with the C bridging header.
- PASS: `sh apps/solidcraft-ipados/scripts/build.sh`; `target/ui-native-build.log`.
- PASS: signed build/install/launch on physical iPad Pro 11-inch (3rd generation),
  iPadOS 27.2. Existing development certificate/wildcard profile reused without
  provisioning updates or global environment/signing changes;
  `target/ui-device-build.log`.
- Physical UIKit clicks/keyboard entry observed: browser open/close retains model
  framing; selecting Post highlights real geometry; Properties opens its radius;
  changing 7 to 12 and Preview displays the evaluated larger solid; Cancel
  restores 7. Selecting the actual reference sketch and Edit sketch switches
  the real planar view and tools; Rectangle dispatch changes the native prompt.
  Visual review led to tighter navigation surfaces, a taller numeric editor and
  removal of unrelated assembly/parameter sections, then successful rebuilds.
- Real Device Hub window screenshots: `/private/tmp/solidcraft-ui-evidence/`.
  These show live physical UIKit/engine geometry, including browser open/closed,
  sketch mode and feature Preview/Cancel. They include Device Hub chrome; they
  are not fabricated framebuffers. No review images are committed or uploaded.
- PASS: physical UIKit `--chrome-check`: four text/floating × light/dark cases;
  actual header/navigation icons retain visible >=44-point touch regions and the
  floating browser keeps its requested frame. Report:
  `target/ui-device-chrome-check.json`. This regression follows a real defect
  observed when switching back to compact mode. The fix resets retained scroll
  offsets after layout and restores manual frame ownership to the browser button.
  The earlier CI attempt was stopped before that source fix and is preserved as
  `target/ui-ci-interrupted-layout-toggle.log`; it is not a completed gate.
  Physical portrait inspection/browser dismissal, light appearance and the legacy
  compatibility layout were also exercised; simultaneous Pencil/VoiceOver remain
  unverified. Final signed build/install/normal launch was captured in
  `13-final-verified-workspace.png` in the evidence directory. Device Hub's compact capture/coordinate path intermittently
  returned `noWindowsAvailable`; full-view screenshots and the native report are
  separate evidence, not repaired/fabricated screenshots.
- PASS: edge-to-edge follow-up unsigned and signed builds:
  `target/ui-edge-to-edge-{build,device-build}.log`. The restricted unsigned
  attempt could not start Xcode's sandbox resource phase; the same established
  build succeeded with required local tool access. No project setting changed.
  Physical UIKit checks pass in light/dark across text/floating switches in both
  orientations: actual canvas `[0,0,1194,834]` in landscape and
  `[0,0,834,1194]` in portrait. Top/bottom native hit tests reach the canvas and
  header/status controls remain inside the safe area. Reports:
  `target/ui-edge-to-edge-chrome-{landscape,portrait}.json`.
  Real screenshots: `target/ui-evidence/14-edge-to-edge-landscape.png` and
  `15-edge-to-edge-portrait.png`. The physical viewer was restored to its original
  landscape orientation. No engine suite or full CI was run for this follow-up.

The automatic full repository run was stopped at the user's instruction on
2026-10-28: **run the engine suite only when the user explicitly requests a full
test**. This instruction supersedes the earlier automatic full-gate-before-commit
rule for this task. Future UI changes use focused native/UI checks. No engine
test, tolerance or CI implementation was changed or disabled.

`target/ui-ci.log` records the interrupted run (exit 143): structural gates and
workspace Clippy passed, as did 54 completed top-level jobs, including the
24-test bridge and the 14-crate wasm check. No completed job failed; the engine
library did not complete. This is **not a completed or green full-CI result**.
B's three accepted failures remain historical baseline observations; they were
not reverified by this interrupted run. The final signed build also passed:
`target/ui-device-final-build.log`.

No simulator was launched. Device Hub pointer/keyboard checks do not establish
physical Pencil, palm rejection, simultaneous fingers/Pencil or VoiceOver
acceptance. Foundation geometry checks cover narrow/keyboard/accessibility sizes;
they are not UIKit hardware tests. Complete hardware coverage remains required.

## Historical UI workstream follow-ups

**The professional dark viewport objective requires B/integrator integration.**
Native chrome is themed, but the actual renderer still uses the default light
palette and an always-on grid. The supplied development assembly screenshot also
used an explicit Top view. UI.md identifies the exact session/viewport/renderer
locations and a display-only palette/guide seam. C leaves those owner-controlled
files intact and adds no fake scene/filter. Dark/light geometry/highlights and a
restrained ordinary 3D grid need that seam and combined verification.

**Project/Files browsing was subsequently integrated.** The completed project
task supplies a library, autosave/restoration and import/export through the
existing native session and upstream file commands. Its implementation and
remaining physical Files-provider checks are documented above and in UI.md.

Review C after its gate, then integrate preserving task history. Reconcile the
small approved Xcode resource additions with other project changes; never replace
shared files wholesale. Integrate B's reviewed render seam and A's service wiring
in P's dependency order. Run D's relevant cross-module regressions, native build
and physical workflows on the combined tree. Run the full gate only on an
explicit full-test request. No automatic baseline/main merge.


---

# Step 8.5 consolidated development baseline

Source baseline **B = `c22acd12702cf3a405df01009b8d8662b3d46365`** is committed on `integration/ipados-foundation`. It is accepted for development under the bounded 2026-10-28 user exception, not certified for release. Tasks A–D are prepared for individually requested launches; this task has not launched them or pushed the branch. Read finalized planning documents from the documentation-only descendant **P** identified in the integrator's final report; create every implementation worktree at B. P is never embedded as its own hash.

The completed full `cargo +1.95.0 xtask ci` run exits **1**: **85/86 top-level checks pass**; the engine library has **2631 passed, 3 failed, 6 ignored**. Only the three unchanged failures in [KNOWN-UPSTREAM-FAILURES.md](KNOWN-UPSTREAM-FAILURES.md) remain under the user exception. All other required checks passed. **CI is not green.**

Completed full-gate log: `/private/tmp/solidcraft-step85-accepted-ci.log`. SHA-256: `c38a8f810f4f554f93fb47468631d48300f4cab250ffe0b8ba81394190e61526`.

## Current verification and preserved work

The full source gate passed 105 session, 24 bridge, 87 desktop-app and 2,932 UI
tests, along with the repository checks, full-workspace Clippy, wasm and doc-test
jobs. The separate web offline/update gate passed in
`/tmp/solidcraft-step85-accepted-web.log`; it is recorded separately because the
full CI command exits at the engine failures before reaching that final stage.
Completed logs are copied to
`target/step8.5-verification/resume-accepted-exceptions/`.

The only source normalization after the source gate removed one redundant EOF
newline from `WorkspaceLayout.swift`; executable Swift tokens did not change.
The exact committed source passes all 186 Foundation checks again in
`/tmp/solidcraft-step85-baseline-native-check.log` and its unsigned device-target
build succeeds in `/tmp/solidcraft-step85-baseline-native-build.log`. The required policy-commit
gate uses this exact source B; its actual result is recorded in P's commit
message and the final report. Its log is `/tmp/solidcraft-step85-policy-ci.log`.

The resumed combined unsigned arm64 Rust/UIKit build passes in `/tmp/solidcraft-step85-resume-native-build.log`; all **186 Foundation cases** pass in `/tmp/solidcraft-step85-resume-native-check.log`. The native English catalog contains **373 keys**. No physical app launch, simulator, installation or signing operation was performed in this consolidation; physical gesture checks remain outstanding.

The baseline preserves the separate user-requested two-finger double-tap Undo and three-finger double-tap Redo work from “Add multitouch undo and redo” (`01a183dd-c2f4-74a1-a93d-024ef30a9e7c`). It uses existing engine history. `SolidCraft.workspace.historyGestures` defaults on and restores the earlier behavior when disabled. The separate UI overhaul task contributed no source changes.

All Steps 1–8 were found and preserved with their real commit ancestry and accumulated implementation. No milestone was recreated, source branch deleted or unrelated upstream update imported. CONSOLIDATION-INVENTORY records provenance; INTERFACES and PARALLEL-DEVELOPMENT define the actual contracts and file ownership.

Tests remain enabled; expectations, tolerances and goldens are unchanged. Any new failed test or changed failure signature blocks acceptance. Recheck the exception after a relevant upstream fix, affected engine/kernel/toolchain changes, and before release. No automatic monitor was created.

The final policy commit P is made after these document edits. Record its full SHA in the final report and verify it is a documentation-only descendant of B. B may retain provisional planning documents; their pending status is superseded only by the finalized policy read from P. No task may substitute moving HEAD or P for the source baseline B.

The prior GPU/CLI proofs and historical device records below retain their original scope. They are not new physical iPad testing. Statements about uncommitted work or pending validation in the historical sections describe those earlier candidates, not B.

## Historical earlier Step 8.5 verification — superseded by the result above

These results predate the preserved gesture work and resumed candidate. They
used Rust `+1.95.0`, the locked `ci` profile and the established Xcode 27 beta
device target; they are not a substitute for the pending fresh validation.
Historical device evidence below is not new verification.

| Check | Observed result |
| --- | --- |
| Session / ABI integration | **PASS:** 105 session and 24 bridge tests, including the new suspension/ordinary-command/cancelled-preview regression. Both focused and final workspace runs pass. |
| Native presentation | **PASS:** 186 Foundation layout/focus/decoder cases; these are not UIKit hit tests. |
| Repository / compiler gates | **PASS:** formatting, assets, icons (17 tests), licences, lists, skills, layers (19 crates), UI containers and full-workspace Clippy `-D warnings`. Seventeen equivalent lint repairs are documented in ARCHITECTURE. |
| Portable compatibility | **PASS:** wasm check for 14 crates, doc-test job, iOS dependency guard (13 shared crates / 177 packages), web offline/update harness, and pending-format merge/fold test. |
| Unsigned native compilation | **PASS:** arm64 Rust archive and UIKit app compile/link; `BUILD SUCCEEDED`. No signing or installation. |
| Engine-backed GPU proofs | **PASS:** 5 sketch, 6 modelling and 11 assembly Mac Metal captures; 8 representative images inspected. Workflows exercise preview/commit/cancel, original identity, regeneration, undo/redo, analytical geometry and native persistence. These are not UIKit/iPad screenshots. |
| Independent CLI reopen | **PASS:** the fresh assembly file evaluates to 4 closed solids, 5,056 mm³, original feature IDs 3/6/9/12, no timeline errors or rollback. |
| Repaired desktop / localization tests | **PASS:** final workspace run has 87 desktop-app tests and 2,932 UI tests passing. This includes the test-only crash-frame fix and catalog isolation/raw native message validation. Focused localization runs also pass 78 UI and 3 xtask tests; all 372 native English messages survive extraction/folding. The optional standalone `xtask i18n` reporter was not run because it forces a separate cold release build; its coverage projection and raw-source preservation tests passed. |
| Complete required gate | **FAILED, historical raw outcome:** first completed full run records 3 engine failures (2,631 engine tests pass, 6 ignored), plus the subsequently repaired crash/catalog failures. The repaired rerun passes 85 of 86 top-level checks: 83 test binaries plus wasm/doc jobs. Its remaining unchanged engine binary again reports the same 3 failed test names; that duplicate process was explicitly stopped with SIGTERM after the earlier checkpoint decision. The rerun exits 1 and is not a completed passing gate. Only the three documented failures are now accepted for development; the new full run remains pending. |
| Physical iPad / simulators | **NOT RUN in Step 8.5.** No physical Pencil/VoiceOver, simulator, signing or installation check. Earlier Step 8 observations remain historical only. |

The three unresolved engine tests are:

- `cmd::features::prism_local_tests::a_ring_cut_round_a_counterbored_hole_in_a_chamfered_disc`:
  removed volume 148.27973047857813 versus 148.44025288211773 mm³.
- `tests::robustness_goldens::seed_36`: invalid cavity ownership, changed topology,
  area and volume; expected successful geometry fails.
- `tests::robustness_goldens::seed_71`: changed area and topology counts.

All three reproduce identically in the preserved preconsolidation test binary.
Its timestamp does not establish an exact source commit. Independent upstream
reports [#6944](https://github.com/tangency-labs/solidcraft/issues/6944),
[#2669](https://github.com/tangency-labs/solidcraft/issues/2669) and
[#3583](https://github.com/tangency-labs/solidcraft/issues/3583) corroborate these
platform problems. No causal integration change was identified. No expected
values, tolerances or golden files were changed. The proposed upstream work is
unmerged and not green at inspection; recording a platform failure does not
repair geometry. The user's limited development exception and exact measurements
are in KNOWN-UPSTREAM-FAILURES; CHECKPOINT preserves the previous decision.


## Historical evidence locations

Complete first gate: `/tmp/solidcraft-step85-ci-resumed.log`.
Repaired rerun, explicitly interrupted engine: `/tmp/solidcraft-step85-ci-final.log`.
Native build: `/tmp/solidcraft-step85-native-build.log`.
Focused/proof logs: `/tmp/solidcraft-step85-{adapter,native-check,web-offline,format-pending,crash-workspace,gpu-sketch,gpu-modelling,gpu-assemblies,cli-compare}.log`.
Older-binary reproductions: `/tmp/solidcraft-step85-{prism,seeds}-before-consolidation.log`.
Copies and images are retained under `target/step8.5-verification/`.
The original 47-file archive and complete history bundle remain under
`target/step8.5-preservation/`; both original and candidate hashes verify.
These are local recovery artifacts, not a remote backup.


---

# Step 8 handoff — assemblies and joints

Continue in this checkout on `feature/ipados`, preserving the uncommitted
Steps 4–7. Step 8 extends the same native workspace, numeric editor, immutable
preview scheduler and shared engine. No reset, upstream merge, dependency
upgrade, native format change or simulator launch occurred. Nothing was pushed.
The historical Step 7/6 records below describe those milestones at their dates;
this section and [ASSEMBLIES.md](ASSEMBLIES.md) give the current assembly status.

## Working assembly workflows

**8A:** Assembly offers native component creation, body ownership transfer,
linked occurrences and independent Paste New. The existing browser displays
component definitions, full occurrence hierarchy, owned bodies and joints.
Choose Occurrence in Select to pick a whole placement, or select its exact path
in the browser. Names are labels; repeated names do not select another instance.
Activate a context explicitly and use Return to root before root authoring.
Rename, native hide/show/isolate, grounding, world/parent-local translation and
rotation about an explicit pivot are integrated. Pending placements are visible;
Capture/Revert applies only to the selected native occurrence IDs. Shared nested
placements retain the engine's sharing behavior and show a notice.

**8B:** Rigid, revolute and slider creation, as-built creation, original-joint
editing, origin/reference repair, limits, suppression and confirmed deletion use
the native commands. Choose both occurrences and geometric/named/explicit-frame
references; inspect their axes, flip, offsets and which part repositions. Preview
uses scratch evaluation and Apply accepts one native transaction. The inspector
shows actual solver values/reasons, including clamping and legitimate no-ops.
Joint driving provides precise entry and touch sliders; Pencil handles change
the same draft parameter and require explicit Apply. A grounded or jointed part
cannot bypass the solver through the placement form.

**8C:** Cylindrical, planar, pin-slot and ball joints use the same creation/edit/
drive workflow. Native floating-chain drag and constrained reach are available
with original occurrence/grab context, residuals and failure feedback. Real joint
frames, native fixed/floating/DOF/conflict/error reports, reference validation,
Undo/Redo and full native persistence are integrated. Existing loops, links,
rigid groups, studies and unexposed assembly data are retained. Their advanced
authoring editors, coupling and motion-study UI are not part of this milestone.

The source sketch and feature timeline remain editable inside their active
component. Shared geometry updates its native instances. Placement changes use
native transforms, not changed body vertices. The renderer reuses source mesh
triangle buffers across occurrence poses and picks/highlights the intended
placement. Edge/sketch streams still upload; per-face/translucent materials use
the existing fallback. No latency or throughput measurement is claimed.

## Semantics and limits

- The checked-out JointOrigin stores an occurrence ID, not a complete path. New
  joints/rebindings reject ambiguous repeated nested endpoints instead of picking
  a first instance. Use a supported independent parent/component copy when that
  context must be independent. Existing native documents remain intact.
- Involved pending placements must be captured/reverted before joint authoring or
  solving. Unrelated pending branches remain untouched. Native sharing notices
  and copy notices are displayed; a full path does not imply instance-local data.
- Named/geometric references are validated using native identity/resolution.
  Missing references do not select nearby replacement geometry. Explicit repair
  updates the original joint ID. Legacy circular references with no endpoint
  retain upstream nearest-end semantics; this is not a persistent-topology fix.
- Limits and placement transforms are native numeric values. Parameter-backed
  offsets, angles and initial joint values retain native expressions; driving a
  DOF follows upstream's evaluated-value behavior. Native constant expression
  spelling can normalize. No separate expression/transform store exists.
- With no joint touching root/grounded geometry, native solving implicitly fixes
  the first active joint's A side for that solve. This does not set grounded.
  The UI reports the native diagnostics instead of inventing DOF counts.
- Occurrence/component deletion, rigid-group creation, relation/coupling/cam and
  motion-study editors are engine-supported but not exposed here. Joint deletion
  warns about the actual removed links; native studies remain and may need repair.
  Full Files/document browsing, coordinated access and recovery remain Step 9.

## Verification — 2026-10-28

Rust `+1.95.0`, locked `ci` profile; Xcode 27 beta / iPhoneOS 27 SDK,
minimum deployment iPadOS 17. Deterministic analytic checks use the real engine
and actual adapter/controller paths, never only screenshots.

| Check | Observed result |
| --- | --- |
| Session + ABI | PASS:105 session tests (23 assembly cases) and23 bridge tests; all earlier sketch/input/modelling cases retained. |
| Shared joint/occurrence regression | PASS:163 joint-filter tests and77 occurrence-filter tests, including both additive API patches. |
| Affected Clippy | PASS:both adapter crates, all targets, `--no-deps -D warnings`. |
| Native deterministic | PASS:186 adaptive layout/focus cases plus production assembly DTO, full-path, slider, refresh and preview-queue fixtures; not UIKit hit tests. |
| Native unsigned build | PASS:arm64 Rust archive and UIKit app compile/link, no simulator. |
| Desktop app compile | PASS:`cargo +1.95.0 check --locked --offline --profile ci -p solidcraft --all-targets`; `/tmp/solidcraft-step8-desktop-check.log`. Compile-only; desktop UI was not exercised. |
| wasm / web offline | PASS:all14 checked crates and actual browser offline/update harness. Shared inherited warnings remain. |
| Dependency guard | PASS:13 shared crates /177 CAD packages; no Apple/frontend leak. |
| Mac GPU | PASS:11 actual model/feedback captures inspected on M3 Pro Metal; hinge, rigid attachment, slider, previews/cancel and native persistence. These are not UIKit screenshots. |
| Native CLI compatibility | PASS:rebuilt shared CLI reads the same four-component/three-joint native file; component definitions, poses, joint frames/limits, feature identities, parameters and metadata match. Sharing and named-expression relationships are covered by the separate session fixtures. Total volume5056mm³. This is CLI validation, not desktop UI testing. |
| Physical iPad | PASS:signed install/launch on physical iPad Pro11-inch(3rd generation), iPadOS27.2. Production UIKit form callbacks create revolute/slider joints, preview/cancel, Apply90°/30°, Undo/Redo, clamp75mm→50mm and save/reopen retaining joint IDs11/12. Matching Metal frames and live device-view screenshots inspected. Human occurrence selection and finger navigation pass per the user. Rotation preview flicker was reproduced and fixed, and the user confirms the updated Pencil preview and cancellation are stable. |
| Full `cargo xtask ci` | BLOCKED:unchanged `crates/geom/src/fill_general.rs:166`, `clippy::nonminimal_bool`. Fmt/assets/icons/licences/lists/skills/layers/UI-containers pass first. No workspace tests run past the Clippy stop. |

Acceptance A covers ownership/history/world-pose preservation, linked versus
independent copies, shared sketch regeneration, repeated nested definitions and
an analytically composed rotated/translated parent. B/C verify the +Z hinge
marker at90° and30°, +X slider displacement20mm, grounded invariance and actual
90°/50mm stops for120°/75mm requests. D covers rotated as-built pose preservation
and native floating rigid-chain movement. E/F cover conflicts, lost references,
original-ID repair, same names/rename, pending/cancel/superseded results,
duplicate Apply, explicit native save/open and further drives. Additional native
joint types each cover creation, edits, invalid inputs, history and roundtrip.

**No commit was made because the mandatory full gate remains red.** No unrelated
lint was edited or suppressed. An independent shared-engine all-target Clippy
also reports inherited `component_clone.rs` collapsible-match and
`sketch_modify.rs` nonminimal-bool lints. No push occurred.

Logs: `/tmp/solidcraft-step8-{adapter,clippy,ci,native-build,native-check,wasm,
web-offline,desktop-check,engine-joints,engine-occurrences,engine-clippy,dependencies,gpu}.log`.
GPU fixture `/tmp/solidcraft-step8-assembly.solidcraft`; captures
`/tmp/solidcraft-step8-assembly-*.png`; rebuilt CLI artifacts/logs use
`/tmp/solidcraft-step8-cli-*`, with comparison in `cli-compare.log`.

## Physical runtime evidence and reproduction

The signed build reuses the existing Xcode-managed development identity/profile;
no provisioning updates, account changes or simulator are involved. DEBUG launch
`--assembly-check --assembly-check-pause` creates a dedicated disposable native
fixture, prints READY only after a matching presented frame, and waits for
SIGUSR1. It never synthesizes a physical Pencil event. Captured stages are hinge
preview90, cancelled0, committed90, committed30, slider75→actual50 and native
reopened history. `--assembly-check` without pause runs the same callback/frame
barriers automatically and leaves the reopened assembly available for hand input.
A 45-second watchdog is a failure deadline, not a sequencing delay.

Evidence: `/tmp/solidcraft-step8-signed-build.log`,
`/tmp/solidcraft-step8-device-check.log`, copied app log
`/tmp/solidcraft-step8-device-native.log`, and native file
`/tmp/solidcraft-step8-ipad-runtime.solidcraft`. The device saves its own unique
Documents/Step8-Assembly-Runtime-UUID.solidcraft, never overwriting the existing
local-design slot. The current shared CLI evaluates that copied physical file as
four closed bodies,6280mm³; original feature IDs3/6/9/10 remain error-free, hinge30
and slider50 world centroids match independent analytic poses within1e-7mm.
`/tmp/solidcraft-step8-ipad-cli-{eval.json,compare.log}` records this independent
check; the earlier four-component/three-joint Mac fixture remains separate.
The physical file's CLI component/joint inspection also verifies definitions
0/1/4/7, full root paths2/5/8, grounded base2, original revolute11/slider12,
slider limits[0,50] and no unresolved/conflicting/floating or pending state.
The final readable-control build repeats the whole driver successfully in
`/tmp/solidcraft-step8-device-final.log`; the signed and unsigned final builds
both succeed. Native label checks cover all34 tool/edit bindings and fallback.

CoreDevice screenshot capture on this iPadOS/Xcode beta combination returns a
wrongly oriented/cropped framebuffer, despite correct1194×834pt scene/window/view
bounds. The user confirmed the real app fills the screen. Raw
`/tmp/solidcraft-step8-ipad-*.png` captures record that service limitation; they
are not evidence of app clipping or complete inspector visibility. All six
actual states were separately captured/inspected in the live physical Device Hub
view via computer use. CoreDevice screen recording reports unsupported on this
device. No image was fabricated, rotated or reconstructed to mask the failure.
The user physically verified occurrence selection and finger navigation, then
reported flicker while rotating with Pencil. A deterministic held-worker test
reproduced the cause: each actual move cleared the preview and flashed the
committed pose. A single display-only last-good scene now remains while the
new value is unvalidated; Apply stays disabled, stale source picks are blocked,
and the latest result replaces it. Cancel/tool/document changes discard it.
The numeric/slider preview path shares that display boundary, including failed
validation, while source selection explicitly removes the retained preview.
The additional test covers coalescing, stale/end-boundary completions, pending
lift, no automatic Apply and history preservation. After installing the fix, the user repeated the ring drag with a physical
Pencil and returned to Properties/Cancel, reporting that the preview is stable.
This verifies the tested hand workflow; simultaneous palm contact, VoiceOver and
device latency remain unmeasured. The final opt-in `--assembly-pencil-check`
prepares the ordinary hinge preview without Apply and leaves its ring visible,
using the same production callbacks and matching-frame barrier.
`/tmp/solidcraft-step8-device-pencil.log` records the fixed-build controller
sequence and ready pose; the hand outcome is the user report, not synthetic
input.

## Changed files and upgrade seams

- Safe adapter: new `assembly.rs` and `assembly_references.rs`; existing
  `lib.rs`, `modelling.rs`, `editing.rs`, `input.rs`, `viewport.rs`, `picking.rs`,
  `workspace.rs`, and the area's existing `tests.rs`. Native records/references
  remain authoritative; presentation caches are revision-aware.
- Bridge/render evidence: existing `renderer.rs`, `ffi/tests.rs` and
  `examples/render_proof.rs`. SharedTris/InstancedMesh use native source meshes
  and complete effective poses; the bounded cache owns no placement semantics.
  ABI3 symbols are unchanged; JSON presentation fields are additive. Rebuild
  Rust and Swift together.
- Native: existing workspace controller/components/layout/session, deterministic
  harness, pending `ipados-assemblies.json`, and scoped host instructions. The
  opt-in DEBUG `--assembly-check` drives production form callbacks and waits for
  exact presented document/revision/preview frames. Device inspection prompted a
  wider text rail, readable panel Close and existing-catalog command-label mapping;
  the locale extractor includes only those13 existing legacy modelling labels.
  `ipados-workspace.json` adds the short Close label with the full accessibility
  label retained. No geometry or command behavior moved into the host.
- Two minimal shared API changes: `cmd/component.rs` adds optional occurrence-ID
  scoping to Capture Position (omission retains capture-all); `cmd/joints.rs`
  adds optional A/B-origin rebinding to joint.edit, validating both before
  mutation and retaining the joint ID. Adapter-only alternatives would capture
  unrelated state or require delete/recreate. Existing area test files cover
  both. No solver, kernel, document structure or format changes. Desktop/web
  callers retain old behavior absent the new parameters; both changes could be
  proposed upstream independently. See ASSEMBLIES and ARCHITECTURE delta ledgers.
- Documentation: ASSEMBLIES, HANDOFF, IMPLEMENTATION, ARCHITECTURE, INPUT, UI,
  BUILDING and native AGENTS. Prior uncommitted Step4–7 work remains intact.

## Exact Step 9 integration points

Reuse `NativeSession`'s serial CAD ownership and current native `file.save`,
`file.save_as` and `doc.open` commands. The existing local slot is Application
Support / SolidCraft / Modelling.solidcraft, storing the full native container.
Replace only the platform access/presentation seam with Files/document services,
security-scoped coordinated reads/writes, recovery and restoration. Do not invent
another design serializer. Preserve unknown component/joint/link/study data and
session visibility. Document replacement must invalidate numeric/model/handle
contexts and late callbacks through the existing document/generation tokens.
Expose pending-placement capture/revert decisions deliberately before saves;
never capture unrelated movement as a side effect of file access. Keep preview
frames separate from committed poses and autosave. Assembly references use
native IDs/full paths; bookmarks and transient browser expansion belong in host
state, not a native CAD format addition. No Step9 browser/recovery work started.

---

## Historical Step 7 record (superseded by Step 8 status above)

# Step 7 handoff — parametric modelling and feature timeline

Continue in this checkout on `feature/ipados`. The preceding uncommitted Steps
4–6 are preserved. Step 7 adds native command-backed modelling forms, a real
editable timeline, transactional previews and app-private native save/reopen.
No branch reset, upstream merge, dependency upgrade, signing/account change or
simulator launch occurred. Nothing was pushed. Commit eligibility and final
verification are recorded below; the Step 6 record remains at the end as history.

## Step 7 working workflows

Choose **Model** outside an active sketch. Pick a classified sketch region,
choose extent/direction, operation and explicit targets, then Preview and Apply.
Pencil can adjust the extrusion distance handle; lift leaves a proposal for
explicit confirmation. Fingers keep the shared navigation path. Tool-first
selection can return to the viewport, then refresh the native input choices.
Lists show their actual sources and support remove/reorder without restarting.

7A is implemented: extrusion with new/join/cut/intersect, positive/negative/
symmetric extents and Through All; fillet/chamfer; original feature editing;
upstream sketch regeneration; expressions/parameters; Undo/Redo and native
persistence. 7B adds revolve, shell, simple holes (blind/Through All), Boolean
union/difference/intersection, body and feature rectangular/circular/path
patterns and mirror. 7C adds basic sweep and ordered-section loft. All use the
existing upstream constructors/evaluator; the exact options and limits are in
[MODELLING.md](MODELLING.md).

Model displays real feature order/IDs, type, suppression, failures/warnings and
stale state. Select a feature for its original parameters, edit/rename/suppress,
inspect dependencies or explicitly confirm native deletion consequences. A
broken downstream feature can open against its real upstream source state and
be repaired by explicit profile/edge/face reselection. Cancel preserves the
failed native history and labels its display accordingly. No visual row-reorder
or unsupported rollback-marker workflow is implied.

Save local design writes the full native container to Application Support /
SolidCraft / Modelling.solidcraft; Reopen local design explicitly confirms
replacement. This intentionally has no Files browser, autosave or restoration.
Unexposed native features and metadata remain in the document. Optional input
names newly introduced by an edit may be allocated by the native reader's
existing name_feature_inputs normalization; existing identities/expressions and
feature order are retained.

## Step 7 verification — 2026-10-28

Explicit Rust `+1.95.0`, locked `ci` profile; Xcode 27 beta / iPhoneOS 27,
minimum iPadOS 17. No physical iPad/Pencil runtime or UIKit screenshots are
claimed. The last recorded device availability remains unavailable.

- Session: **82/82 tests passed**, including 23 new Step 7 cases and all earlier
  input/workspace/sketch regressions. The modelling name filter matches 25 cases
  because two earlier cases already used that word.
- Acceptance A: dimensioned 40 × 30 sketch → 10 mm extrusion, 12,000 mm³;
  original depth edit15 → 18,000 mm³; original width edit60 → 27,000 mm³;
  original sketch/extrusion IDs retained, bounds, Undo/Redo and native roundtrip.
- Acceptance B: explicit top-plane circle and negative cut through15;
  27,000 − 375π then radius edit3 → 27,000 − 135π mm³. Both original
  extrusions remain editable. Native kernel-backed mass checks compare removed
  volume within the repository's curved-geometry tolerance (relative5e-4), not
  screenshot/tessellation appearance. Boxes use absolute1e-6 mm³.
- C/D: independent fillet/chamfer/shell, revolve, hole, all Booleans, body/feature/
  path patterns, mirror, sweep and loft cover creation, original-feature editing,
  invalid inputs, result structure, Undo/Redo and native roundtrip. Analytic
  checks include annular revolution, rectangular patterns and square frusta.
- F: real source-aware synthetic Pencil handle replay includes navigation,
  repeated drags, cancellation, resize, current-preview validation and explicit
  confirmation. Deterministic completion tests reject older work, document
  changes and duplicate confirmation. Mixed-body picks and empty targets during
  an existing feature edit are rejected. Invalid references cannot silently
  adopt a different edge/face/profile; explicit repair retains the same ID.
- Native Foundation checks: **176 adaptive layout/focus cases** plus production
  modelling-response decoding, exact UInt64 reference and nested-expression
  preservation passed. These are not UIKit hit tests.
- Mac GPU: actual native modelling controller preview10 → committed10 →
  original depth15 → Undo/Redo and a named-radius cut captured and inspected, including the actual
  extrusion handle. Native container saved for independent CLI validation.
  These are model/feedback captures, not screenshots of the iPad interface.

Final checks:

| Check | Observed result |
| --- | --- |
| Session + ABI | PASS:82 session +19 bridge tests, including earlier milestones. |
| Affected Clippy | PASS:both adapter crates, all targets, ci, --no-deps -D warnings. |
| Full wasm | PASS:all14 checked crates, including the web frontend; inherited shared warnings remain. |
| Web offline | PASS:actual existing offline/update browser harness. |
| Dependency guard | PASS:13 shared crates /177 CAD packages, no platform/frontend leak. |
| Native build | PASS:Rust arm64 archive plus unsigned UIKit app compile/link; no simulator or app launched. |
| Native deterministic | PASS:176 adaptive cases plus production modelling DTO/exact-reference/expression fixtures. |
| GPU | PASS:6 actual model/feedback captures inspected, including the cut; no UIKit screenshot claim. |
| Native CLI compatibility | PASS:upstream CLI reopened the same four-feature native design, found one closed body, retained both sketches and dimensions, bore_radius and the radius expression bore_radius, all four stable IDs, no timeline error/warning. Volume17,575.9554388 mm³ versus analytic18,000−135π=17,575.8849918 (within curved tolerance); bounds40×30×15. This is CLI validation, not desktop UI testing. |
| Full cargo xtask ci | BLOCKED:unchanged crates/geom/src/fill_general.rs:166, clippy::nonminimal_bool. Fmt/assets/icons(17 tool tests)/licences/lists/skills/layers/UI containers passed first. Workspace Rust tests do not run beyond this blocker; focused tests and full wasm were run independently. |

**No commit was made because the mandatory full gate remains red.** No shared
lint was suppressed or modified to make this platform milestone appear green.
Nothing was pushed. The branch still includes the preserved uncommitted prior
milestones. The final unsigned product is
`target/ipados-xcode/Build/Products/Debug-iphoneos/SolidCraft.app`.

Logs use `/tmp/solidcraft-step7-{adapter,bridge,clippy,ci,native-build,native-check,
wasm,web-offline,gpu,cli}.log`. Captures:
`/tmp/solidcraft-step7-model-{preview10,depth10,depth15,undo,redo,cut3}.png`.
Native CLI fixture: `/tmp/solidcraft-step7-model.solidcraft`.

## Step 7 changed-file map and upgrade seams

- Safe adapter: new `crates/ipados/session/src/modelling.rs` and `references.rs`;
  existing `lib.rs`, `editing.rs`, `input.rs`, `viewport.rs`, `workspace.rs` and
  the existing `tests.rs`. Document/session ownership and history stay upstream.
- ABI: existing `bridge/src/ffi.rs`, `ffi/tests.rs`, C header, and the existing
  `examples/render_proof.rs`. ABI3 symbols remain; JSON `kind:model` adds typed
  begin/inspect/preview/commit/cancel requests. Rebuild Rust and Swift together.
- Native host: existing workspace controller/components/layout/session;
  locale extractor, existing deterministic harness and pending
  `assets/locales/pending/ipados-modelling.json`. App-private file services call
  native `file.save`/`doc.open`; no CAD logic moved into Swift.
- Documentation: MODELLING, IMPLEMENTATION, HANDOFF, ARCHITECTURE and BUILDING;
  scoped native language exception explicitly includes Step7 presentation.
- No Step7 shared engine/kernel/document/solver source, dependency or native
  format patch was needed. Upstream maintenance checks are command parameter
  shapes (not all schemas are complete), feature-definition shapes, strict
  profile/naming APIs, scratch/ComputedCommands and the native writer/reader.
  No copied upstream frontend module exists.

## Remaining limits and Step 8 integration

Richer extrusion To/From Object/Two Sides/draft controls, advanced fillet styles,
specialised hole variants, sweep/loft rails/apex controls, arbitrary reference
axis/plane creation and native timeline reorder/rollback controls remain
unexposed. Existing representations are preserved. Actual kernel diagnostics
limit individual operations; no new algorithm or mesh substitute fills a gap.
No group is reduced to a static tool palette; supported basic workflows in
7A/7B/7C have tested creation/editing/history/persistence paths.

Physical UIKit/Pencil/touch/palm-rejection/keyboard/VoiceOver acceptance and
screenshots remain outstanding. Validate the actual controls on the available
physical iPad before describing hardware acceptance as complete. Performance
latency has not been measured. Assemblies/joints and document browsing were not
implemented here.

Step8 should reuse the one Session owner, authoritative body/feature/occurrence
identities, shared parameter editor, model draft tickets and immutable preview
publication. Do not create a parallel assembly document/history. Read
MODELLING.md, ARCHITECTURE.md and this handoff first.

---

## Historical Step 6 record (superseded by Step 7 status above)

# Step 6 handoff — sketch editor

Continue in this checkout on `feature/ipados` (two commits ahead of origin), with
Steps 4–5 and Step 6 preserved as uncommitted work. No reset, branch switch,
upstream merge, dependency upgrade or global toolchain/signing change occurred.
No commit: the mandatory repository gate still fails on the inherited shared
Clippy lint below. Nothing was pushed.

## Start here

- [SKETCHING.md](SKETCHING.md): capability inventory, actual command bindings,
  gestures, solver/dimension semantics, preview/history and acceptance coverage.
- [IMPLEMENTATION.md](IMPLEMENTATION.md): completed scope and changed-file map.
- [ARCHITECTURE.md](ARCHITECTURE.md), [INPUT.md](INPUT.md), [UI.md](UI.md): retained
  engine ownership, camera/input, workspace and expression-editor contracts.
- [BUILDING.md](BUILDING.md): build and deterministic check commands.

## Working workflow

Normal launch retains the real development fixture. Choose **Create sketch**,
select a datum, existing construction plane or suitable selected planar face,
inspect the plane preview, then confirm. The original native plane/attachment
value is retained. Model → select a sketch → **Edit sketch** reopens its existing
IDs and history. Finish works for open or under-constrained sketches.

The sketch palette offers Select, lines/chains, two-point rectangle, circle,
three-point analytic arc, fit-point spline and Move/handles. Pencil places or
drags; fingers pan/zoom in the active plane. Finish chain completes a spline or
stops a line chain. Cancel discards only the current provisional operation.
Properties supplies compatible constraints, selected/related constraint
inspection/removal and actual native solver/profile state. Driving dimensions
use the shared expression editor, explicit Preview/Apply/Cancel and retained
parameter expressions. Pencil can place dimensions and move annotation text.

Secondary native operations include Trim/Extend, construction conversion,
Translate/Copy/Rotate/Rotate Copy and Mirror. Transform expression fields use
upstream unit parsing and the same scratch proposal/acceptance path. Whole-sketch
Delete is explicit and queries native dependents before confirmation.

No new engine, solver, kernel, document format, history or topology model exists.
`SceneSnapshot.sketch` carries the actual immutable native sketch/world plane;
`ToolOperation::Sketch` extends the existing latest-work mailbox. Engine work
runs without the viewport/render lock, including initial dimension preparation.
Stale interaction/selection/lifecycle tokens are checked before publication or
acceptance. Primitive previews evaluate through the scratch sketch; bounded
native live drags defer downstream regeneration. Completed operations regenerate
the original timeline. Error paths retain the previous valid document/preview.

## Observed verification — 2026-10-28

Explicit Rust `+1.95.0`, locked/offline profile `ci`; Xcode 27 beta / iPhoneOS 27,
minimum iPadOS 17. This task did not boot or launch a simulator.

| Check | Actual result |
| --- | --- |
| Session/ABI tests | PASS: 59 session + 15 bridge tests; includes the earlier Steps 3–5 regressions. |
| Rectangle + native downstream | PASS: origin connection, native closed profile, 40 × 30 dimensions, reopen/change width60 while height30 remains; 10 mm prism 12,000 → 18,000 mm³ within 1e-6 mm³; undo/redo and native container round trip/edit. |
| Constraints/failure | PASS: solver-aware drag retains relationships; genuine contradictory existing dimension versus fixed geometry is rejected with valid expression/document/history retained; recovery after unfix; duplicate/invalid selections and degenerate geometry. |
| Curves/input | PASS: separate circle/arc/spline creation/handles/history/cancel; shared-endpoint chain/profile closure; Pencil dimension/text placement; stale results, predicted input, finger navigation, resizing and focus cancellation. |
| Plane/compatibility | PASS: tilted plane and existing construction reference; exact certified face attachment follows source height change10→20 and survives reopening/container load; unexposed ellipse/IDs preserved. |
| Secondary | PASS: native trim, extend to a real crossing, transform proposal/undo/cancel; native response-decoder fixtures cover transform completion and dependencies. |
| Native deterministic | PASS: 176 adaptive layout/focus cases plus production command/preview response fixtures. These are not UIKit hit tests. |
| Affected Clippy | PASS: both adapter crates, all targets, `--no-deps -D warnings`. |
| Wasm adapters | PASS; three inherited shared-engine dead-code warnings. |
| Device dependency guard | PASS: 13 shared crates / 177 resolved CAD packages; no frontend/platform leak. |
| Final unsigned native build | PASS: Rust arm64 archive and UIKit app compile/link against iPhoneOS SDK. |
| macOS GPU evidence | PASS: actual adapter Pencil rectangle → driving dimensions → reopen → width60 preview/commit → undo/redo, with native profile area1,800 mm². Before/after captures inspected; these are model/feedback captures, not UIKit screenshots. |
| Web offline compatibility | PASS: actual upstream offline/update browser harness. |
| Full `cargo xtask ci` | BLOCKED at unchanged `crates/geom/src/fill_general.rs:166`, `clippy::nonminimal_bool`. Fmt, assets, icons (17 tool tests), licences, lists, skills, layers and UI containers passed before Clippy. |

Logs: `/tmp/solidcraft-step6-{adapter,clippy,wasm,ci,native-build,native-check,web-offline}.log`.
GPU proof: `/tmp/solidcraft-step6-gpu.log` and
`/tmp/solidcraft-step6-sketch-{40x30,60x30-preview,60x30,undo,redo}.png`.
Unsigned app: `target/ipados-xcode/Build/Products/Debug-iphoneos/SolidCraft.app`.
The full workspace tests/wasm and desktop smoke did not run past the inherited
full-gate blocker; adapter wasm and web-offline were checked separately. No shared
lint was changed or suppressed to make this milestone appear green.

## Device coverage and remaining limits

The user stated the physical iPad is currently unavailable. An intermediate
signed build succeeded using the existing certificate/profile; installation and
runtime verification were not completed. The install/launch attempt timed out in
CoreDevice before verification, and device work stopped at that instruction.
**No Step 6 UIKit screenshots, human UI taps, physical Pencil/touch/palm rejection,
VoiceOver or latency measurement are claimed.** Build-only and deterministic
coverage above must not be presented as complete physical acceptance.

DEBUG `--sketch-check` is compiled but unrun. It waits for presented frames and
completions, injects software Pencil events through the real native boundary,
creates/dimensions a rectangle, finishes/reopens, edits width, undoes/redoes and
checks native area1,800 mm²/expression. It leaves the actual dimension inspector
visible for later screenshot review. It does not prove Pencil hardware. When the
device returns, perform the A–E workflows through the actual controls, capture
wide/compact/portrait screenshots and verify keyboard/VoiceOver and simultaneous
Pencil/finger input. Do not silently substitute this driver for those checks.

Additional native variants remain unexposed: centre arcs, control-point spline
authoring/insertion, path trim, transform scale and richer automatic tangency.
Control-point spline insertion is an actual upstream limitation. Other variants
are native UI scope gaps. General solid modelling/timeline UI, assemblies, Files,
autosave/restoration and the native document browser were not added.

## Exact Step 7 integration points

- Reuse `SessionAdapter::execute`, upstream command schemas/history and the same
  serial engine owner. General extrusion UI is not present; tests use native
  `solid.extrude {sketch,distance}`. The rectangle fixture proves regeneration.
- Read actual feature IDs/ownership from `workspace.rows`; extend presentation
  around existing timeline inspection/edit/dependents APIs, preserving references.
- Reuse native workspace panels, `NumericExpressionEditor`, focus policy and
  production `EngineCommandEnvelope`. Keep expressions intact until confirmation.
- Reuse `ToolWork`/scratch/ComputedCommands and latest-work/version tickets; do not
  evaluate CAD while holding viewport/render locks or accept discarded work as a
  successful commit. Keep commands/camera/cancelled previews in their own history
  contracts. Retain native selection/entity types that lack controls.
- Rebuild both Rust and Swift after protocol changes. ABI 3 C symbols remain;
  JSON adds `sketch_plane`, `sketch_plane_cancel`, dimension numeric context,
  sketch workspace/annotations and tool proposals. Header documents the boundary.
- Resolve the inherited full-gate blocker in a separate reviewed change before a
  commit. Preserve this dirty workspace and its preceding implementation.


## Continuous Pencil extrusion resizing — 2026-10-28

This local fix addresses the report that an existing extrusion jumps during a
Pencil handle drag and repeatedly needs to be grabbed. No commit, push, signing,
installation or device launch was performed by this task.

Reproduced before the fix: the native 128-sample mailbox cancelled a straight
Pencil burst after capacity was reached, dropping contact ownership; tapping a
modelling handle left a Candidate capture after lift; a second grab was rejected
while the first lifted contact still awaited geometry. Stationary actual samples
also kept superseding otherwise useful in-flight previews.

Changes are confined to the iPad host/adapters and existing tests/proof example:

- `InputSample` moved unchanged from `MetalCanvas.swift`, and `InputMailbox` moved
  from `NativeSession.swift`, into Foundation-only `WorkspaceLayout.swift` for
  production-code tests. At capacity, the mailbox removes redundant intermediate
  actual Pencil moves along the same screen path within a quarter display point.
  It retains begin/end/cancel, final position, reversal and context boundaries;
  uncompressible input still cancels conservatively. The bound remains128.
- `session/src/input.rs` releases tap-only handle capture, permits a fresh grab
  after lift while an old preview is pending, and ignores unchanged handle values.
  Old interaction results remain rejected. Existing Apply/one-step history,
  document/revision, navigation suspension and cancellation rules remain intact.
- Existing session/ABI tests cover quick re-grabs, stale previous results,
  stationary samples, continuous multi-preview resizing with form inspection,
  real geometry and one explicit Apply history step. The native harness covers
  1,000-sample straight/reversing bursts, ordering, final lift, the unchanged
  uncompressible-overload fallback and stale mailbox generations.

Verification completed:

- `cargo +1.95.0 test --locked --profile ci -p solidcraft-ipados-session -p solidcraft-ipados-bridge --lib`:
  PASS,125 session and28 bridge tests at this check. Log:
  `/tmp/solidcraft-continuous-resize-tests.log`.
- `sh apps/solidcraft-ipados/scripts/check-workspace.sh`: PASS,330 adaptive cases
  plus production input/backlog checks and native icon checks. macOS file
  coordination required running the harness outside the restricted sandbox.
- Affected all-targets Clippy with `-D warnings`, and the updated render proof's
  Clippy: PASS. Logs `/tmp/solidcraft-continuous-resize-clippy.log` and
  `/tmp/solidcraft-continuous-resize-proof-clippy.log`.
- `sh apps/solidcraft-ipados/scripts/build.sh`: PASS, unsigned device-target build,
  after rerunning outside the restricted runner so Xcode could initialize its
  build-script sandbox. Log `/tmp/solidcraft-continuous-resize-build.log`.
- Both iPad crates' wasm32 check: PASS. Three pre-existing dead-code warnings in
  shared engine file/slicer code remain. Log `/tmp/solidcraft-continuous-resize-wasm.log`.
- `git diff --check` and `cargo +1.95.0 fmt --all -- --check`: PASS.

The existing `render_proof` example adds `--continuous-resize`. A real shared-Metal
capture replayed tap/re-grab then one Pencil contact10→15→20→12 mm with preview
publication and form inspection between moves. Analytical volumes were12,000,
18,000,24,000 and14,400 mm³; authoritative geometry stayed12,000 until explicit
Apply. Captures `/tmp/solidcraft-continuous-resize-{depth10,depth15,depth20,depth12,applied12}.png`
were produced; depth10/depth20/depth12 were visually inspected. The unchanged
camera naturally clips the taller20 mm preview at its top edge. These are Mac GPU
captures with synthetic input, not UIKit screenshots or physical Pencil evidence.
Log: `/tmp/solidcraft-continuous-resize-gpu.log`. Physical iPad/Pencil retesting
remains required; no simulator was used. Full repository CI was not run by this
fix task because no commit was requested; the integration owner must run its
required final gates before committing the combined workspace.

## Selected edge length and automatic extrusion depth — 2026-10-31

Source candidate `70afff53de8485659e218ab3b96d3b1222e9d6e5` contains
edge-length commit `564d557a` and the separate automatic-extrusion-distance commit
`70afff53`. The checkout was clean for the reserved physical iPad session.

Selecting a part edge now shows its length in millimetres in the bottom status
area. The latest selected edge supplies the readout; clearing selection or
selecting another entity removes it. Circular rims use the existing engine's
circumference, arcs its arc length, and other edges its measured length, through
`inspect.measure` on scratch. Tools → Show selected edge length disables the
readout (`SolidCraft.workspace.edgeLengthReadout`, default enabled).

Verification:

- Two focused `workspace_edge_length` session tests passed: 40/30 mm selection
  updates, clear/non-edge/unavailable selection, unchanged document/history and
  the analytic circumference of a radius-7 mm circular rim.
- The existing bridge workspace test passed. Native workspace/icon checks passed
  all 692 adaptive cases, including localised numeric formatting and the disabled
  preference. Rust formatting, affected adapter Clippy, session wasm32 check,
  lists and i18n passed. The wasm check retained an existing engine dead-code
  warning. No engine suite, full CI or simulator was run.
- Unsigned and signed device-target builds passed. Signing reused the existing
  team/profile; no provisioning/account settings changed. Xcode build scripts and
  native file-coordination harness required execution outside the sandbox.
- Reserved device check `--edge-length-check` passed on the physical iPad:
  synthetic Pencil samples selected a 40 mm edge, added a 30 mm edge, then cleared
  selection. All three UIKit screenshots were captured and visually inspected;
  bottom captions were `Edge length: 40 mm`, `Edge length: 30 mm` and
  `Model · No selection`. The check also verified disabling the readout.
- During the same reservation, `--viewport-check --viewport-check-hold-face
  --auto-face-extrude-check` opened the production Extrude form and logged
  `Automatic extrusion user distance check PASS: empty distance, source-surface
  handle, unvalidated draft`. This is console evidence, not a native extrusion
  screenshot. Normal app launch was restored and the iPad reservation released.

Local evidence is under `target/edge-length-device-20261031/`: signed build log,
`build.json` with candidate and signed binary hashes, `ipad-session.json`,
`edge-length-check.json`, three `edge-length-*.png` captures, console log and
reservation receipt. The app's optional Info.plist candidate stamp was absent;
the clean source candidate and signed binary hashes bind this build to the test.
These checks verify physical iPad runtime with synthetic input; physical finger
and Pencil contact remain unverified. Nothing was pushed.

## History navigation preserves undo and redo — 2026-10-31

Source candidate `058b71a33a0904140dfbe73c6ac4582c8e9efa62` keeps iPad history
navigation outside the undo/redo stacks. The adapter temporarily retains both
engine stacks while executing `timeline.roll_to` or `timeline.rollback`, then
restores them, including on failure. Marker evaluation, geometry publication,
model edits and persistence still use the existing engine.

Both added session regressions failed before the fix. All 211 session and 30
bridge tests pass, as do affected adapter Clippy, Rust formatting, the wasm32
session check, 692 native workspace cases, icon checks and unsigned/signed iPad
builds. The wasm check retains an existing engine dead-code warning. No engine
test suite, full CI or simulator was run. Xcode resource generation and the
native file-coordination harness required execution outside the outer sandbox.

The reserved physical iPad `--history-check` passed using production UIKit
controls on a disposable fixture: backward/forward arrows, reverse scrubbing,
the empty stage and final lift preserved pending redo; Redo restored a feature
rename, Undo reversed that edit, and a new rename discarded the redo branch.
Twelve native screenshots were captured; the empty, single-body and completed
undo/redo stages were visually inspected. Input was synthetic UIKit actions;
physical finger/Pencil contact remains unverified. Normal launch was restored
and the device reservation released.

Evidence is under `target/history-navigation-device-20261031/`, including signed
build output, `build.json` binding the clean candidate to signed binary hashes,
installation/launch records, console acceptance, screenshots, `ipad-session.json`
and `reservation.log`. Signing reused the existing team and profile. Fix commit
`058b71a3` is local on `develop`; nothing was pushed.
