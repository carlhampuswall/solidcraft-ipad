# iPadOS Rust adapters

`session` owns the existing engine behind a safe Rust command/snapshot boundary.
`bridge` supplies the narrow native C ABI and shared GPU renderer integration.
See [architecture](../../docs/ipados/ARCHITECTURE.md) and
[build instructions](../../docs/ipados/BUILDING.md).

Adapters call the existing engine command registry and document timeline. The
geometry kernel and sketch solver remain in their upstream crates. Shared crates
must never depend on an iPad adapter.

This namespace is excluded from the `crates/*` workspace glob because it has no
manifest; both actual packages are registered explicitly. Follow the
[setup](../../docs/ipados/SETUP.md) and [engineering rules](../../docs/ipados/UPSTREAM.md).
