// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::Ordering;
use std::convert::{TryFrom, TryInto};

use cid::Cid;
use libipld_core::ipld::Ipld;
use once_cell::unsync::OnceCell;
use serde::de::{self, DeserializeOwned};
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};

use super::node::Node;
use super::{Error, Hash, HashAlgorithm, KeyValuePair};
use crate::Config;

#[doc(hidden)]
pub mod version {
    #[derive(PartialEq, Eq, Debug)]
    pub struct V0;
    #[derive(PartialEq, Eq, Debug)]
    pub struct V3;

    pub trait Version {
        const NUMBER: usize;
    }

    impl Version for V0 {
        const NUMBER: usize = 0;
    }

    impl Version for V3 {
        const NUMBER: usize = 3;
    }
}

/// Pointer to index values or a link to another child node.
#[derive(Debug)]
pub(crate) enum Pointer<K, V, H, Ver = version::V3> {
    Values(Vec<KeyValuePair<K, V>>),
    Link {
        cid: Cid,
        cache: OnceCell<Box<Node<K, V, H, Ver>>>,
    },
    Dirty(Box<Node<K, V, H, Ver>>),
}

impl<K: PartialEq, V: PartialEq, H, Ver> PartialEq for Pointer<K, V, H, Ver> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Pointer::Values(a), Pointer::Values(b)) => a == b,
            (Pointer::Link { cid: a, .. }, Pointer::Link { cid: b, .. }) => a == b,
            (Pointer::Dirty(a), Pointer::Dirty(b)) => a == b,
            _ => false,
        }
    }
}

mod pointer_v0 {
    use cid::Cid;
    use serde::{Deserialize, Serialize};

    use crate::KeyValuePair;

    use super::Pointer;

    #[derive(Serialize)]
    pub(super) enum PointerSer<'a, K, V> {
        #[serde(rename = "0")]
        Link(&'a Cid),
        #[serde(rename = "1")]
        Vals(&'a [KeyValuePair<K, V>]),
    }

    #[derive(Deserialize, Serialize)]
    pub(super) enum PointerDe<K, V> {
        #[serde(rename = "0")]
        Link(Cid),
        #[serde(rename = "1")]
        Vals(Vec<KeyValuePair<K, V>>),
    }

    impl<'a, K, V, H, Ver> TryFrom<&'a Pointer<K, V, H, Ver>> for PointerSer<'a, K, V> {
        type Error = &'static str;

        fn try_from(pointer: &'a Pointer<K, V, H, Ver>) -> Result<Self, Self::Error> {
            match pointer {
                Pointer::Values(vals) => Ok(PointerSer::Vals(vals.as_ref())),
                Pointer::Link { cid, .. } => Ok(PointerSer::Link(cid)),
                Pointer::Dirty(_) => Err("Cannot serialize cached values"),
            }
        }
    }

    impl<K, V, H, Ver> From<PointerDe<K, V>> for Pointer<K, V, H, Ver> {
        fn from(pointer: PointerDe<K, V>) -> Self {
            match pointer {
                PointerDe::Link(cid) => Pointer::Link {
                    cid,
                    cache: Default::default(),
                },
                PointerDe::Vals(vals) => Pointer::Values(vals),
            }
        }
    }
}

/// Serialize the Pointer like an untagged enum.
impl<K, V, H, Ver> Serialize for Pointer<K, V, H, Ver>
where
    K: Serialize,
    V: Serialize,
    Ver: self::version::Version,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match Ver::NUMBER {
            0 => pointer_v0::PointerSer::try_from(self)
                .map_err(ser::Error::custom)?
                .serialize(serializer),
            _ => match self {
                Pointer::Values(vals) => vals.serialize(serializer),
                Pointer::Link { cid, .. } => cid.serialize(serializer),
                Pointer::Dirty(_) => Err(ser::Error::custom("Cannot serialize cached values")),
            },
        }
    }
}

impl<K, V, H, Ver> TryFrom<Ipld> for Pointer<K, V, H, Ver>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
{
    type Error = String;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        match ipld {
            ipld_list @ Ipld::List(_) => {
                let values: Vec<KeyValuePair<K, V>> = from_ipld(ipld_list)?;
                Ok(Self::Values(values))
            }
            Ipld::Link(cid) => Ok(Self::Link {
                cid,
                cache: Default::default(),
            }),
            other => Err(format!(
                "Expected `Ipld::List` or `Ipld::Link`, got {:#?}",
                other
            )),
        }
    }
}

/// Deserialize the Pointer like an untagged enum.
impl<'de, K, V, H, Ver> Deserialize<'de> for Pointer<K, V, H, Ver>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
    Ver: self::version::Version,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Ver::NUMBER {
            0 => {
                let pointer_de: pointer_v0::PointerDe<K, V> =
                    Deserialize::deserialize(deserializer)?;
                Ok(Pointer::from(pointer_de))
            }
            _ => Ipld::deserialize(deserializer)
                .and_then(|ipld| ipld.try_into().map_err(de::Error::custom)),
        }
    }
}

impl<K, V, H, Ver> Default for Pointer<K, V, H, Ver> {
    fn default() -> Self {
        Pointer::Values(Vec::new())
    }
}

impl<K, V, H, Ver> Pointer<K, V, H, Ver>
where
    K: Serialize + DeserializeOwned + Hash + PartialOrd,
    V: Serialize + DeserializeOwned,
    H: HashAlgorithm,
{
    pub(crate) fn from_key_value(key: K, value: V) -> Self {
        Pointer::Values(vec![KeyValuePair::new(key, value)])
    }

    /// Internal method to cleanup children, to ensure consistent tree representation
    /// after deletes.
    pub(crate) fn clean(&mut self, conf: &Config, depth: u32) -> Result<(), Error> {
        match self {
            Pointer::Dirty(n) => match n.pointers.len() {
                0 => Err(Error::ZeroPointers),
                _ if depth < conf.min_data_depth => {
                    // We are in the shallows where we don't want key-value pairs, just links,
                    // so as long as they are pointing at non-empty nodes we can keep them.
                    // The rest of the rules would move key-value pairs up.
                    Ok(())
                }
                1 => {
                    // Node has only one pointer, swap with parent node
                    if let Pointer::Values(vals) = &mut n.pointers[0] {
                        // Take child values, to ensure canonical ordering
                        let values = std::mem::take(vals);

                        // move parent node up
                        *self = Pointer::Values(values)
                    }
                    Ok(())
                }
                i if 2 <= i && i <= conf.max_array_width => {
                    // If more child values than max width, nothing to change.
                    let mut children_len = 0;
                    for c in n.pointers.iter() {
                        if let Pointer::Values(vals) = c {
                            children_len += vals.len();
                        } else {
                            return Ok(());
                        }
                    }
                    if children_len > conf.max_array_width {
                        return Ok(());
                    }

                    // Collect values from child nodes to collapse.
                    let mut child_vals: Vec<KeyValuePair<K, V>> = n
                        .pointers
                        .iter_mut()
                        .filter_map(|p| {
                            if let Pointer::Values(kvs) = p {
                                Some(std::mem::take(kvs))
                            } else {
                                None
                            }
                        })
                        .flatten()
                        .collect();

                    // Sorting by key, values are inserted based on the ordering of the key itself,
                    // so when collapsed, it needs to be ensured that this order is equal.
                    child_vals.sort_unstable_by(|a, b| {
                        a.key().partial_cmp(b.key()).unwrap_or(Ordering::Equal)
                    });

                    // Replace link node with child values
                    *self = Pointer::Values(child_vals);
                    Ok(())
                }
                _ => Ok(()),
            },
            _ => unreachable!("clean is only called on dirty pointer"),
        }
    }
}

fn from_ipld<T: DeserializeOwned>(ipld: Ipld) -> Result<T, String> {
    Deserialize::deserialize(ipld).map_err(|error| error.to_string())
}
