#![cfg_attr(not(feature = "std"), no_std)]

pub mod abi;
pub mod block_context;
pub mod block_execution;
pub mod execution;
pub mod fee;
pub mod state;
#[cfg(any(feature = "testing", test))]
pub mod test_utils;
pub mod transaction;
pub mod utils;

#[cfg(feature = "parity-scale-codec")]
pub mod scale_codecs;

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

mod sync {
    #[cfg(feature = "std")]
    pub use std::sync::{Arc, Mutex, MutexGuard};

    #[cfg(not(feature = "std"))]
    pub use crate::without_std::sync::*;
}

mod cmp {
    #[cfg(not(feature = "std"))]
    pub use core::cmp::min;
    #[cfg(feature = "std")]
    pub use std::cmp::min;
}
