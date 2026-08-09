mod advisory_locks;
pub mod vfs_backed;

#[cfg(not(target_family = "wasm"))]
mod native;

#[cfg(not(target_family = "wasm"))]
pub use native::*;
#[cfg(target_family = "wasm")]
pub use vfs_backed::*;

#[cfg(test)]
mod tests;
