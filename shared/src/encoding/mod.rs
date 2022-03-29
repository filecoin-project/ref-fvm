// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod bytes;
mod cbor;
mod errors;
mod hash;
mod vec;

pub use serde::{de, ser};
pub use serde_bytes;
pub use serde_ipld_dagcbor::{from_reader, from_slice, to_writer};

pub use self::bytes::*;
pub use self::cbor::*;
pub use self::errors::*;
pub use self::hash::*;
pub use self::vec::*;

// TODO: these really don't work all that well in a shared context like this as anyone importing
// them also need to _explicitly_ import the serde_tuple & serde_repr crates. These are _macros_,
// not normal items.

pub mod tuple {
    pub use serde_tuple::{self, Deserialize_tuple, Serialize_tuple};
}

pub mod repr {
    pub use serde_repr::{Deserialize_repr, Serialize_repr};
}

// TODO: upstream this. Upstream doesn't allow encoding unsized types (e.g., slices).

/// Serializes a value to a vector.
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>, Error>
where
    T: ser::Serialize + ?Sized,
{
    let mut vec = Vec::new();
    value.serialize(&mut serde_ipld_dagcbor::Serializer::new(&mut vec))?;
    Ok(vec)
}
