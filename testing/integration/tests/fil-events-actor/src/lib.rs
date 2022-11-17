#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

#[cfg(target_arch = "wasm32")]
mod actor;

#[cfg(target_arch = "wasm32")]
pub use actor::invoke;
