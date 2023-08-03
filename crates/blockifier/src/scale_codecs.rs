use core::hash::Hash;
use parity_scale_codec::{Decode, Encode, EncodeAsRef};

use crate::stdlib::collections::HashSet;
use crate::stdlib::vec::Vec;

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

impl<T: Encode + Decode + Eq + Hash> From<HashSetCodec<T>> for HashSet<T> {
    fn from(value: HashSetCodec<T>) -> Self {
        value.0.into_iter().collect()
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
