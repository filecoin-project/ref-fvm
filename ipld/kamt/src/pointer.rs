// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::Ordering;

use cid::Cid;
use fvm_ipld_encoding::{BytesDe, BytesSer};
use once_cell::unsync::OnceCell;
use serde::{ser, Deserialize, Deserializer, Serialize, Serializer};

use super::node::Node;
use super::{Error, KeyValuePair};
use crate::bitfield::Bitfield;
use crate::ext::Extension;
use crate::Config;

/// Pointer to index values or a link to another child node.
#[derive(Debug)]
pub(crate) enum Pointer<K, V, H, const N: usize> {
    Values(Vec<KeyValuePair<K, V>>),
    Link {
        cid: Cid,
        ext: Extension,
        cache: OnceCell<Box<Node<K, V, H, N>>>,
    },
    Dirty {
        node: Box<Node<K, V, H, N>>,
        ext: Extension,
    },
}

impl<K: PartialEq, V: PartialEq, H, const N: usize> PartialEq for Pointer<K, V, H, N> {
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

/// Serialize the Pointer like a tagged enum.
impl<K, V, H, const N: usize> Serialize for Pointer<K, V, H, N>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        enum PointerSer<'a, K, V> {
            #[serde(rename = "v")]
            Values(&'a [KeyValuePair<K, V>]),
            #[serde(rename = "l")]
            Link(&'a Cid, u32, BytesSer<'a>),
        }
        match self {
            Pointer::Values(vals) => PointerSer::Values(vals),
            Pointer::Link { cid, ext, .. } => {
                PointerSer::Link(cid, ext.len(), BytesSer(ext.path_bytes()))
            }
            Pointer::Dirty { .. } => {
                return Err(ser::Error::custom("Cannot serialize cached values"))
            }
        }
        .serialize(serializer)
    }
}

/// Deserialize the Pointer like a tagged enum.
impl<'de, K, V, H, const N: usize> Deserialize<'de> for Pointer<K, V, H, N>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum PointerDe<K, V> {
            #[serde(rename = "v")]
            Values(Vec<KeyValuePair<K, V>>),
            #[serde(rename = "l")]
            Link(Cid, u32, BytesDe),
        }
        match PointerDe::<K, V>::deserialize(deserializer)? {
            PointerDe::Values(vals) => Ok(Self::Values(vals)),
            PointerDe::Link(k, l, BytesDe(p)) => Ok(Self::Link {
                cid: k,
                ext: Extension::new(l, p),
                cache: Default::default(),
            }),
        }
    }
}

impl<K, V, H, const N: usize> Default for Pointer<K, V, H, N> {
    fn default() -> Self {
        Pointer::Values(Vec::new())
    }
}

impl<K, V, H, const N: usize> Pointer<K, V, H, N>
where
    K: PartialOrd,
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

                    match &mut n.pointers[0] {
                        Pointer::Values(vals) => {
                            // Take child values, to ensure canonical ordering
                            let values = std::mem::take(vals);

                            // move parent node up
                            *self = Pointer::Values(values)
                        }
                        Pointer::Link {
                            cid,
                            ext: ext2,
                            cache,
                        } => {
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
                        } => {
                            let ext = unsplit_ext(conf, &n.bitfield, ext1, ext2)?;
                            *self = Pointer::Dirty {
                                node: std::mem::take(sub),
                                ext,
                            }
                        }
                    }
                    Ok(())
                }
                w if 2 <= w && w <= conf.max_array_width => {
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

/// Helper method to undo a former split.
fn unsplit_ext(
    conf: &Config,
    bf: &Bitfield,
    parent_ext: &Extension,
    child_ext: &Extension,
) -> Result<Extension, Error> {
    // Figure out which bucket contains the pointer.
    let idx = bf.last_one_idx().ok_or(Error::ZeroPointers)? as u8;

    let idx = Extension::from_idx(idx, conf.bit_width);
    let ext = Extension::unsplit(parent_ext, &idx, child_ext)?;

    Ok(ext)
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use fvm_ipld_encoding::{to_vec, BytesSer};
    use serde::Serialize;

    use crate::ext::Extension;
    use crate::pointer::Pointer;
    use crate::KeyValuePair;

    fn check_encoding<T, V>(expected: &T, input: &V)
    where
        T: Serialize,
        V: Serialize,
    {
        let encoded_expected = to_vec(&expected).unwrap();
        let encoded_input = to_vec(&input).unwrap();
        assert_eq!(encoded_expected, encoded_input);
    }

    #[test]
    fn test_values_representation() {
        use crate::id::Identity;

        let v: Pointer<&str, &str, Identity, 32> =
            Pointer::Values(vec![KeyValuePair("foo", "bar")]);
        check_encoding(
            // Expect a map with "v" -> [("foo", "bar")]
            &[("v", [("foo", "bar")])]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            &v,
        )
    }

    #[test]
    fn test_link_representation() {
        use crate::id::Identity;

        // Random values.
        let k = cid::Cid::new_v1(
            0x1,
            cid::multihash::Multihash::wrap(0x2, &[0xa; 32]).unwrap(),
        );
        let ext = Extension::new(20, vec![0xff; 3]);

        let v: Pointer<&str, &str, Identity, 32> = Pointer::Link {
            ext,
            cid: k,
            cache: Default::default(),
        };

        check_encoding(
            // Expect a map with "l" -> (cid, ext_len, ext_bytes)
            // note: BytesDe will (correctly) reject "lists" of bytes, only accepting cbor "bytes" objects.
            &[("l", (k, 20, BytesSer(&[0xff; 3][..])))]
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            &v,
        )
    }
}
