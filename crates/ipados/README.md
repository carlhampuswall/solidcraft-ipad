# Future iPadOS Rust adapters

Reserved namespace for small iPad-specific Rust integration crates, such as an
engine bridge or Apple platform service adapter. Add a crate only when a concrete
integration needs it; no bridge, FFI dependency or replacement CAD code is added now.

Adapters call the existing engine command registry and document timeline. The
geometry kernel and sketch solver remain in their upstream crates. Shared crates
must never depend on an iPad adapter.

This namespace is excluded from the `crates/*` workspace glob because it has no
manifest. When implementing a nested crate, add its exact path to the root
workspace members and verify it appears in `cargo metadata --no-deps`. Follow the
[setup](../../docs/ipados/SETUP.md) and [engineering rules](../../docs/ipados/UPSTREAM.md).
