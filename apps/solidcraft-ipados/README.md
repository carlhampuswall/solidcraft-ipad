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
