pub mod with_std {
    pub use std::{any, borrow, boxed, fmt, mem, num, ops, rc, string, sync, vec};

    pub mod collections {
        pub use std::collections::{HashMap, HashSet};
    }
}
