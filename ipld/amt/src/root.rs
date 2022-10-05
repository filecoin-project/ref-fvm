// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use serde::de::{self, Deserialize};
use serde::ser::{self, Serialize};

use crate::node::CollapsedNode;
use crate::{init_sized_vec, Node, DEFAULT_BIT_WIDTH};

const V0: u8 = 0;
const V3: u8 = 3;

/// Root of an AMT vector, can be serialized and keeps track of height and count
pub(super) type Root<V> = RootImpl<V, 3>;

pub(super) type Rootv0<V> = RootImpl<V, 0>;

#[derive(PartialEq, Debug)]
pub(crate) struct RootImpl<V, const VER: u8 = V3> {
    pub bit_width: u32,
    pub height: u32,
    pub count: u64,
    pub node: Node<V>,
}

impl<V> RootImpl<V, V3> {
    pub(super) fn new(bit_width: u32) -> Self {
        Self {
            bit_width,
            count: 0,
            height: 0,
            node: Node::Leaf {
                vals: init_sized_vec(bit_width),
            },
        }
    }
}

impl<V> RootImpl<V, V0> {
    pub(super) fn new() -> Rootv0<V> {
        Self {
            bit_width: crate::DEFAULT_BIT_WIDTH,
            count: 0,
            height: 0,
            node: Node::Leaf {
                vals: init_sized_vec(crate::DEFAULT_BIT_WIDTH),
            },
        }
    }
}

impl<V, const VER: u8> Serialize for RootImpl<V, VER>
where
    V: Serialize,
{
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match VER {
            // legacy amt v0 doesn't serialize bit_width as DEFAULT_BIT_WIDTH is used.
            V0 => (&self.height, &self.count, &self.node).serialize(s),
            V3 => (&self.bit_width, &self.height, &self.count, &self.node).serialize(s),
            _ => unreachable!(),
        }
    }
}

impl<'de, V> Deserialize<'de> for RootImpl<V, V3>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (bit_width, height, count, node): (_, _, _, CollapsedNode<V>) =
            Deserialize::deserialize(deserializer)?;
        Ok(Self {
            bit_width,
            height,
            count,
            node: node.expand(bit_width).map_err(de::Error::custom)?,
        })
    }
}

// Deserialize impl for legacy amt v0
impl<'de, V> Deserialize<'de> for RootImpl<V, V0>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        // legacy amt v0 doesn't include bit_width as DEFAULT_BIT_WIDTH is used.
        let (height, count, node): (_, _, CollapsedNode<V>) =
            Deserialize::deserialize(deserializer)?;
        Ok(Self {
            bit_width: crate::DEFAULT_BIT_WIDTH,
            height,
            count,
            node: node
                .expand(crate::DEFAULT_BIT_WIDTH)
                .map_err(de::Error::custom)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use fvm_ipld_encoding::{from_slice, to_vec};

    use super::*;

    #[test]
    fn serialize_symmetric() {
        let mut root = Root::new(0);
        root.height = 2;
        root.count = 1;
        root.node = Node::Leaf { vals: vec![None] };
        let rbz = to_vec(&root).unwrap();
        assert_eq!(from_slice::<Root<String>>(&rbz).unwrap(), root);
    }

    #[test]
    fn serialize_deserialize_legacy_amt() {
        let mut root: Rootv0<_> = Rootv0::new();
        root.height = 2;
        root.count = 1;
        root.node = Node::Leaf {vals: vec![None]};
        let rbz = to_vec(&root).unwrap();
        assert_eq!(from_slice::<Rootv0<String>>(&rbz).unwrap(), root); // FIXME: fails currently
    }
}
