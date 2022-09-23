// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::hash::Hasher;
use std::{mem, slice};

pub mod hash_algo;
pub mod key;

pub use hash_algo::{HashAlgorithm, HashedKey};
pub use key::BytesKey;

/// Custom trait to avoid issues like https://github.com/rust-lang/rust/issues/27108.
pub trait Hash {
    fn hash(&self, state: &mut dyn Hasher);

    fn hash_slice(data: &[Self], state: &mut dyn Hasher)
    where
        Self: Sized,
    {
        for piece in data {
            piece.hash(state);
        }
    }
}

macro_rules! impl_write {
    ($(($ty:ident, $meth:ident),)*) => {$(
        impl Hash for $ty {
            fn hash(&self, state: &mut dyn Hasher) {
                state.$meth(*self)
            }

            fn hash_slice(data: &[$ty], state: &mut dyn Hasher) {
                let newlen = data.len() * mem::size_of::<$ty>();
                let ptr = data.as_ptr() as *const u8;
                state.write(unsafe { slice::from_raw_parts(ptr, newlen) })
            }
        }
    )*}
}

impl_write! {
    (u8, write_u8),
    (u16, write_u16),
    (u32, write_u32),
    (u64, write_u64),
    (usize, write_usize),
    (i8, write_i8),
    (i16, write_i16),
    (i32, write_i32),
    (i64, write_i64),
    (isize, write_isize),
    (u128, write_u128),
    (i128, write_i128),
}

impl Hash for bool {
    fn hash(&self, state: &mut dyn Hasher) {
        state.write_u8(*self as u8)
    }
}

impl Hash for char {
    fn hash(&self, state: &mut dyn Hasher) {
        state.write_u32(*self as u32)
    }
}

impl Hash for str {
    fn hash(&self, state: &mut dyn Hasher) {
        state.write(self.as_bytes());
    }
}

impl Hash for String {
    fn hash(&self, state: &mut dyn Hasher) {
        state.write(self.as_bytes());
    }
}

macro_rules! impl_hash_tuple {
    () => (
        impl Hash for () {
            fn hash(&self, _state: &mut dyn Hasher) {}
        }
    );

    ( $($name:ident)+) => (
        impl<$($name: Hash),*> Hash for ($($name,)*) where last_type!($($name,)+): ?Sized {
            #[allow(non_snake_case)]
            fn hash(&self, state: &mut dyn Hasher) {
                let ($(ref $name,)*) = *self;
                $($name.hash(state);)*
            }
        }
    );
}

macro_rules! last_type {
    ($a:ident,) => { $a };
    ($a:ident, $($rest_a:ident,)+) => { last_type!($($rest_a,)+) };
}

impl_hash_tuple! {}
impl_hash_tuple! { A }
impl_hash_tuple! { A B }
impl_hash_tuple! { A B C }
impl_hash_tuple! { A B C D }
impl_hash_tuple! { A B C D E }
impl_hash_tuple! { A B C D E F }
impl_hash_tuple! { A B C D E F G }
impl_hash_tuple! { A B C D E F G H }
impl_hash_tuple! { A B C D E F G H I }
impl_hash_tuple! { A B C D E F G H I J }
impl_hash_tuple! { A B C D E F G H I J K }
impl_hash_tuple! { A B C D E F G H I J K L }

impl<T: Hash> Hash for [T] {
    fn hash(&self, state: &mut dyn Hasher) {
        Hash::hash_slice(self, state)
    }
}

impl<T: Hash> Hash for Vec<T> {
    fn hash(&self, state: &mut dyn Hasher) {
        Hash::hash_slice(self, state)
    }
}

impl<T: ?Sized + Hash> Hash for &T {
    fn hash(&self, state: &mut dyn Hasher) {
        (**self).hash(state);
    }
}

impl<T: ?Sized + Hash> Hash for &mut T {
    fn hash(&self, state: &mut dyn Hasher) {
        (**self).hash(state);
    }
}

impl<T: ?Sized> Hash for *const T {
    fn hash(&self, state: &mut dyn Hasher) {
        if mem::size_of::<Self>() == mem::size_of::<usize>() {
            // Thin pointer
            state.write_usize(*self as *const () as usize);
        } else {
            // Fat pointer
            let (a, b) = unsafe { *(self as *const Self as *const (usize, usize)) };
            state.write_usize(a);
            state.write_usize(b);
        }
    }
}

impl<T: ?Sized> Hash for *mut T {
    fn hash(&self, state: &mut dyn Hasher) {
        if mem::size_of::<Self>() == mem::size_of::<usize>() {
            // Thin pointer
            state.write_usize(*self as *const () as usize);
        } else {
            // Fat pointer
            let (a, b) = unsafe { *(self as *const Self as *const (usize, usize)) };
            state.write_usize(a);
            state.write_usize(b);
        }
    }
}
