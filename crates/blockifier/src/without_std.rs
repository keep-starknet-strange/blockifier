#[macro_use]
extern crate alloc;

pub mod without_std {
    pub use alloc::{borrow, boxed, rc, string, vec};
    pub use core::{any, fmt, mem, num, ops};

    pub mod collections {
        pub use hashbrown::{HashMap, HashSet};
    }

    pub mod cached {
        use super::collections::HashMap;

        #[derive(Clone, Debug)]
        pub struct SizedCache<K, V>(HashMap<K, V>);

        pub trait Cached<K, V> {
            fn cache_get(&mut self, class_hash: &K) -> Option<&V>;
            fn cache_set(&mut self, class_hash: K, contract_class: V);
            fn with_size(size: usize) -> Self;
            fn cache_size(&self) -> usize;
            fn cache_hits(&self) -> Option<usize>;
        }

        impl<K: core::hash::Hash + Eq, V: core::clone::Clone> Cached<K, V> for SizedCache<K, V> {
            fn cache_get(&mut self, class_hash: &K) -> Option<&V> {
                self.0.get(class_hash)
            }

            fn cache_set(&mut self, class_hash: K, contract_class: V) {
                self.0.insert(class_hash, contract_class);
            }

            fn with_size(size: usize) -> Self {
                SizedCache(HashMap::with_capacity(size))
            }

            fn cache_size(&self) -> usize {
                1 // TODO
            }
            fn cache_hits(&self) -> Option<usize> {
                Some(1) // TODO
            }
        }
    }

    pub mod sync {
        pub use alloc::sync::Arc;
        pub use spin::{Mutex, MutexGuard};
    }
}
