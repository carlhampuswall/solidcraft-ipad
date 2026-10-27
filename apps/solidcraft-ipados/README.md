# SolidCraft for iPadOS

Native UIKit lifecycle and Metal-layer host for the shared SolidCraft Rust engine.
The application target is iPad only, with an iPadOS 17.0 deployment target.

The small proof runs the existing engine command registry, evaluates real model
geometry and draws through the shared Rust GPU renderer. Start with
[building](../../docs/ipados/BUILDING.md), [architecture](../../docs/ipados/ARCHITECTURE.md) and the
[permanent engineering rules](../../docs/ipados/UPSTREAM.md#permanent-engineering-rules).

Build without signing or launching anything:

```sh
sh apps/solidcraft-ipados/scripts/build.sh
```

Open `SolidCraft.xcodeproj` for physical-device deployment after supplying your own
signing team. No team, credentials, profiles or signing identity are committed.
This directory stays excluded from Cargo: the Rust packages live in `crates/ipados/`.
Step 2 forwards typed Pencil/finger samples; gestures and interactive modelling
belong to Step 3.
