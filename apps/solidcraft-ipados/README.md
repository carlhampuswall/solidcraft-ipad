# SolidCraft for iPadOS

Native UIKit lifecycle and Metal-layer host for the shared SolidCraft Rust engine.
The application target is iPad only, with an iPadOS 17.0 deployment target.

The interactive viewport runs the existing engine registry, evaluates real
parametric geometry and draws through the shared Rust GPU renderer. Start with
[building](../../docs/ipados/BUILDING.md), [architecture](../../docs/ipados/ARCHITECTURE.md) and the
[permanent engineering rules](../../docs/ipados/UPSTREAM.md#permanent-engineering-rules).

Build without signing or launching anything:

```sh
sh apps/solidcraft-ipados/scripts/build.sh
```

Open `SolidCraft.xcodeproj` for physical-device deployment after supplying your own
signing team. No team, credentials, profiles or signing identity are committed.
This directory stays excluded from Cargo: the Rust packages live in `crates/ipados/`.
Step 3 supplies real topology selection, camera navigation and a command-generated
fixture with live feature edits and undo/redo. Detailed Pencil/touch arbitration
belongs to Step 4. See the handoff for automated, build and device verification.

## Quick access toolbars

Hold a tool icon on either vertical bar, then drag up or down to reorder it.
Neighbors slide into place and the bar scrolls when the held icon reaches an
edge. Drop the icon on **Remove from quick access** to move it into the bar's
deeper menu. Use **Add to quick access** in that menu to bring a tool back;
quick tools are omitted from the deeper tool lists. Each bar saves its own
order across launches, including tools used only while sketching.

The deeper menu stays available even after every quick tool is removed.
**Reset quick access** restores that bar's defaults. The settings menu's
**Customizable toolbars** switch restores the previous fixed bars when off.
VoiceOver offers **Move up**, **Move down** and **Remove from quick access**
actions. Animations respect Reduce Motion.

The macOS workspace harness checks saved ordering, removal, addition, overflow
exclusion, corrupt preferences and default-tool upgrades. Debug launch argument
`--quick-access-check` additionally exercises the production drag, removal,
addition and cancellation paths on both bars, restores the original preferences,
and writes `quick-access-check.json` plus `quick-access-*.png` captures in Documents.
That diagnostic uses synthetic calls; physical contact delivery is separate.
