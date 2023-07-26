use cairo_vm::vm::runners::cairo_runner::ExecutionResources;
use parity_scale_codec::{Decode, Encode, EncodeAsRef};
use std::collections::HashSet;
use std::hash::Hash;
use std::iter::FromIterator;

#[derive(Encode, Decode)]
pub struct USizeCodec(u64);

impl From<USizeCodec> for usize {
    fn from(value: USizeCodec) -> Self {
        value.0 as usize
    }
}

impl From<&usize> for USizeCodec {
    fn from(value: &usize) -> Self {
        Self(*value as u64)
    }
}

impl EncodeAsRef<'_, usize> for USizeCodec {
    type RefType = USizeCodec;
}

#[derive(Encode, Decode)]
pub struct HashSetCodec<T>(Vec<T>)
where
    T: Encode + Decode;

impl<T: Encode + Decode + Clone + Eq + Hash> From<HashSetCodec<T>> for HashSet<T> {
    fn from(value: HashSetCodec<T>) -> Self {
        Self::from_iter(value.0.iter().cloned())
    }
}

impl<T: Encode + Decode + Clone> From<&HashSet<T>> for HashSetCodec<T> {
    fn from(value: &HashSet<T>) -> Self {
        Self(value.iter().cloned().collect())
    }
}

impl<'a, T: 'a + Encode + Decode + Clone> EncodeAsRef<'a, HashSet<T>> for HashSetCodec<T> {
    type RefType = HashSetCodec<T>;
}

#[derive(Encode, Decode)]
pub struct ExecutionResourcesCodec {
    pub n_steps: u64,
    pub n_memory_holes: u64,
    pub builtin_instance_counter: Vec<(String, u64)>,
}

impl From<ExecutionResourcesCodec> for ExecutionResources {
    fn from(value: ExecutionResourcesCodec) -> Self {
        Self {
            n_steps: value.n_steps as usize,
            n_memory_holes: value.n_memory_holes as usize,
            builtin_instance_counter: value
                .builtin_instance_counter
                .into_iter()
                .map(|(k, v)| (k, v as usize))
                .collect(),
        }
    }
}

impl From<&ExecutionResources> for ExecutionResourcesCodec {
    fn from(value: &ExecutionResources) -> Self {
        Self {
            n_steps: value.n_steps as u64,
            n_memory_holes: value.n_memory_holes as u64,
            builtin_instance_counter: value
                .builtin_instance_counter
                .clone()
                .into_iter()
                .map(|(k, v)| (k, v as u64))
                .collect(),
        }
    }
}

impl EncodeAsRef<'_, ExecutionResources> for ExecutionResourcesCodec {
    type RefType = ExecutionResourcesCodec;
}
