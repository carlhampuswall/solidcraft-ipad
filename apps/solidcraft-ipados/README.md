# SolidCraft for iPadOS

Reserved for the future native iPadOS application, Apple project configuration,
platform resources and packaging. No interface or application target exists yet.

Use the existing Rust engine through narrow adapters; keep CAD logic out of the
platform host. Start with [setup](../../docs/ipados/SETUP.md) and the
[permanent engineering rules](../../docs/ipados/UPSTREAM.md#permanent-engineering-rules).

This directory is excluded from the Rust workspace while it has no `Cargo.toml`.
Revisit that exclusion only if this directory becomes an actual Cargo package.
