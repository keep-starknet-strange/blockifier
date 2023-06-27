#![cfg_attr(not(feature = "std"), no_std)]

pub mod abi;
pub mod block_context;
pub mod execution;
pub mod fee;
pub mod state;
pub mod transaction;
pub mod utils;

#[cfg(test)]
pub mod test_utils;

#[cfg(feature = "std")]
include!("./with_std.rs");

#[cfg(not(feature = "std"))]
include!("./without_std.rs");

pub mod stdlib {
    #[cfg(feature = "std")]
    pub use crate::with_std::*;
    #[cfg(not(feature = "std"))]
    pub use crate::without_std::*;
}
