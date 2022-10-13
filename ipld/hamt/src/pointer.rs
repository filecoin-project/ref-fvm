// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};

use cid::Cid;
use fvm_ipld_encoding::ser::Error as EncodingError;
use libipld_core::ipld::Ipld;
use libipld_core::serde::to_ipld;
use once_cell::unsync::OnceCell;
use serde::de::{self, DeserializeOwned};
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};

use super::node::Node;
use super::{Error, Hash, HashAlgorithm, KeyValuePair, MAX_ARRAY_WIDTH};
use crate::bitfield::Bitfield;
use crate::ext::Extension;
use crate::Config;

/// Pointer to index values or a link to another child node.
#[derive(Debug)]
pub(crate) enum Pointer<K, V, H> {
    Values(Vec<KeyValuePair<K, V>>),
    Link {
        cid: Cid,
        ext: Option<Extension>,
        cache: OnceCell<Box<Node<K, V, H>>>,
    },
    Dirty {
        node: Box<Node<K, V, H>>,
        ext: Option<Extension>,
    },
}

impl<K: PartialEq, V: PartialEq, H> PartialEq for Pointer<K, V, H> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&Pointer::Values(ref a), &Pointer::Values(ref b)) => a == b,
            (
                &Pointer::Link {
                    cid: ref a,
                    ext: ref e1,
                    ..
                },
                &Pointer::Link {
                    cid: ref b,
                    ext: ref e2,
                    ..
                },
            ) => a == b && e1 == e2,
            (
                &Pointer::Dirty {
                    node: ref a,
                    ext: ref e1,
                },
                &Pointer::Dirty {
                    node: ref b,
                    ext: ref e2,
                },
            ) => a == b && e1 == e2,
            _ => false,
        }
    }
}

/// Serialize the Pointer like an untagged enum.
impl<K, V, H> Serialize for Pointer<K, V, H>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Pointer::Values(vals) => vals.serialize(serializer),
            Pointer::Link { cid, ext: None, .. } => cid.serialize(serializer),
            Pointer::Link {
                cid, ext: Some(e), ..
            } => {
                // Using a `Map` and not a tuple so it's easy to distinguish from the case of `Values`.
                // Constructing the map manually so we don't have to clone the extension and give it to a struct.
                let mut map = BTreeMap::new();
                add_to_ipld_map::<S, _>(&mut map, "c", cid)?;
                add_to_ipld_map::<S, _>(&mut map, "e", e)?;
                Ipld::Map(map).serialize(serializer)
            }
            Pointer::Dirty { .. } => Err(ser::Error::custom("Cannot serialize cached values")),
        }
    }
}

impl<K, V, H> TryFrom<Ipld> for Pointer<K, V, H>
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
                ext: None,
                cache: Default::default(),
            }),
            Ipld::Map(mut map) => {
                let cid: Cid = from_ipld_map(&mut map, "c")?;
                let ext: Extension = from_ipld_map(&mut map, "e")?;

                Ok(Self::Link {
                    cid,
                    ext: Some(ext),
                    cache: Default::default(),
                })
            }
            other => Err(format!(
                "Expected `Ipld::List`, `Ipld::Map` and `Ipld::Link`, got {:#?}",
                other
            )),
        }
    }
}

/// Deserialize the Pointer like an untagged enum.
impl<'de, K, V, H> Deserialize<'de> for Pointer<K, V, H>
where
    K: DeserializeOwned,
    V: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ipld::deserialize(deserializer).and_then(|ipld| ipld.try_into().map_err(de::Error::custom))
    }
}

impl<K, V, H> Default for Pointer<K, V, H> {
    fn default() -> Self {
        Pointer::Values(Vec::new())
    }
}

impl<K, V, H> Pointer<K, V, H>
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
            Pointer::Dirty { node: n, ext: ext1 } => match n.pointers.len() {
                0 => Err(Error::ZeroPointers),
                _ if depth < conf.min_data_depth => {
                    // We are in the shallows where we don't want key-value pairs, just links,
                    // so as long as they are pointing at non-empty nodes we can keep them.
                    // The rest of the rules would either move key-value pairs up, or undo a split.
                    // But if we use extensions and minimum data depth, splits will only happen after
                    // the minimum data depth as well, and these don't need undoing. So we can skip.
                    Ok(())
                }
                1 => {
                    // Node has only one pointer, swap with parent node
                    // If all `self` does is Link to `n`, and all `n` does is Link to `sub`, and we're using extensions,
                    // then `self` could Link to `sub` directly. `n` was most likely the result of a split, but one of
                    // the nodes it pointed at had been removed since.
                    let can_have_splits = conf.use_extensions || conf.push_data_to_leaves;

                    match &mut n.pointers[0] {
                        Pointer::Values(vals) if !conf.push_data_to_leaves => {
                            // Take child values, to ensure canonical ordering
                            let values = std::mem::take(vals);

                            // move parent node up
                            *self = Pointer::Values(values)
                        }
                        Pointer::Link {
                            cid,
                            ext: ext2,
                            cache,
                        } if can_have_splits => {
                            // Replace `self` with a
                            let ext = unsplit_ext(conf, &n.bitfield, ext1, ext2)?;
                            *self = Pointer::Link {
                                cid: *cid,
                                ext,
                                cache: std::mem::take(cache),
                            }
                        }
                        Pointer::Dirty {
                            node: sub,
                            ext: ext2,
                        } if can_have_splits => {
                            let ext = unsplit_ext(conf, &n.bitfield, ext1, ext2)?;
                            *self = Pointer::Dirty {
                                node: std::mem::take(sub),
                                ext,
                            }
                        }
                        _ => (),
                    }
                    Ok(())
                }
                2..=MAX_ARRAY_WIDTH if !conf.push_data_to_leaves => {
                    // If more child values than max width, nothing to change.
                    let mut children_len = 0;
                    for c in n.pointers.iter() {
                        if let Pointer::Values(vals) = c {
                            children_len += vals.len();
                        } else {
                            return Ok(());
                        }
                    }
                    if children_len > MAX_ARRAY_WIDTH {
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

fn from_ipld_map<T: DeserializeOwned>(
    map: &mut BTreeMap<String, Ipld>,
    key: &str,
) -> Result<T, String> {
    let ipld = map
        .remove(key)
        .ok_or_else(|| format!("`{key}` not found in map."))?;

    from_ipld(ipld)
}

fn add_to_ipld_map<S: Serializer, T: Serialize>(
    map: &mut BTreeMap<String, Ipld>,
    key: &str,
    value: &T,
) -> Result<(), S::Error> {
    let value =
        to_ipld(value).map_err(|e| S::Error::custom(format!("cannot serialize `{key}`: {e}")))?;
    map.insert(key.to_owned(), value);
    Ok(())
}

/// Helper method to undo a former split.
fn unsplit_ext(
    conf: &Config,
    bf: &Bitfield,
    parent_ext: &Option<Extension>,
    child_ext: &Option<Extension>,
) -> Result<Option<Extension>, Error> {
    // Figure out which bucket contains the pointer.
    let idx = bf
        .last_one_idx()
        .expect("There is supposed to be exactly one pointer") as u8;

    let idx = Extension::from_idx(idx, conf.bit_width);
    let ext = Extension::unsplit(parent_ext, &idx, child_ext)?;
    Ok(Some(ext))
}
