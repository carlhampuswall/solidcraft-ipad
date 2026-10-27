//! Native host boundary. CAD semantics live in the safe session adapter.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![deny(unsafe_code)]

pub use solidcraft_ipados_session::{AdapterError, RenderSnapshot, SessionAdapter};

#[cfg(not(target_arch = "wasm32"))]
mod ffi;
#[cfg(not(target_arch = "wasm32"))]
pub mod renderer;
