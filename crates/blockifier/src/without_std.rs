#[macro_use]
extern crate alloc;

pub mod without_std {
    pub use alloc::{borrow, boxed, rc, string, sync, vec};
    pub use core::{any, fmt, mem, num, ops};

    pub mod collections {
        pub use hashbrown::{HashMap, HashSet};
    }
}
