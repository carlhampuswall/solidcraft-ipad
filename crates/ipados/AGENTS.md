# iPad Rust integration

Read ../../docs/ipados/ARCHITECTURE.md and the root skills before changes.
Keep CAD semantics in the shared engine; do not copy upstream modules here.
`session` remains safe Rust with no Apple dependencies and builds for wasm.

The user approved a narrow native ABI exception for Step 2. `bridge/src/ffi.rs`
alone may allow unsafe code for C byte buffers, C exports and creation of a wgpu
surface from a host-retained CAMetalLayer. Document every safety precondition in
the C header and beside each unsafe operation. No unsafe engine/session code,
raw engine pointers, foreign-owned Rust allocations, or unchecked handle casts.
The bridge root denies unsafe; all other modules remain safe Rust.
