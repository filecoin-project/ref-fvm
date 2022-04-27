// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use serde::de::{self, Deserialize};
use serde::ser::{self, Serialize};

use crate::node::CollapsedNode;
use crate::{init_sized_vec, Node};

/// Root of an AMT vector, can be serialized and keeps track of height and count
#[derive(PartialEq, Debug)]
pub(super) struct Root<V, const S: usize> {
    pub bit_width: u32,
    pub height: u32,
    pub count: u64,
    pub node: Node<V, S>,
}

impl<V, const S: usize> Root<V, S> {
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

impl<V, const S: usize> Serialize for Root<V, S>
where
    V: Serialize,
{
    fn serialize<Ser>(&self, s: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: ser::Serializer,
    {
        (&self.bit_width, &self.height, &self.count, &self.node).serialize(s)
    }
}

impl<'de, V, const S: usize> Deserialize<'de> for Root<V, S>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let (bit_width, height, count, node): (_, _, _, CollapsedNode<V, S>) =
            Deserialize::deserialize(deserializer)?;
        Ok(Self {
            bit_width,
            height,
            count,
            node: node.expand(bit_width).map_err(de::Error::custom)?,
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
}
