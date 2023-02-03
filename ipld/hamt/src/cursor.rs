// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

/// A Path is specified as a sequence of branches taken at each level of traversal
#[derive(Default, PartialEq, Eq, Clone, Debug)]
pub(crate) struct Path(pub(crate) Vec<u8>);

/// A NodeCursor points to a non-leaf node reached by following the specified path from the root of
/// a trie at the `root` cid
#[derive(Default, PartialEq, Eq, Clone, Debug)]
pub(crate) struct NodeCursor {
    path: Path,
}

/// A LeafCursor points to a leaf node reached by following the specified path from the root of a
/// trie at the `root` cid
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct LeafCursor {
    path: Path,
}

/// BranchOrdering is the result of comparing two `Branches` in a tree-like where each represents a
/// path to a node in the tree
enum BranchOrdering {
    Less,
    Equal,
    Greater,
    Ancestor,
    Descendant,
}

impl Path {
    /// Compares two Paths. A path "A" is less than a path "B" if "A" is contained within a subtree
    /// to the left of "B" (when considered from their common ancestor).
    fn cmp(&self, other: &Self) -> BranchOrdering {
        // compare along shared segment length
        for (branch, gate) in self.0.iter().zip(other.0.iter()) {
            match branch.cmp(gate) {
                std::cmp::Ordering::Less => {
                    return BranchOrdering::Less;
                }
                std::cmp::Ordering::Greater => {
                    return BranchOrdering::Greater;
                }
                std::cmp::Ordering::Equal => {
                    // path is equal to the range start so far, continue checking further branches
                }
            }
        }

        // the entire path segments matched, so the paths are of the same lineage
        match self.0.len().cmp(&other.0.len()) {
            std::cmp::Ordering::Less => BranchOrdering::Ancestor,
            std::cmp::Ordering::Greater => BranchOrdering::Descendant,
            std::cmp::Ordering::Equal => BranchOrdering::Equal,
        }
    }
}

impl NodeCursor {
    /// Creates a new cursor, extending the path by the specified `branch`
    pub fn create_branch(&self, branch: u8) -> NodeCursor {
        let mut new_path = self.path.clone();
        new_path.0.push(branch);
        NodeCursor { path: new_path }
    }

    /// Creates a leaf cursor, extending the path by the specified `branch`
    pub fn create_leaf(&self, branch: u8) -> LeafCursor {
        let mut new_path = self.path.clone();
        new_path.0.push(branch);
        LeafCursor { path: new_path }
    }

    /// Returns true if this branch can be safely skipped, given the specified `range_start`.
    /// Direct ancestors of the range start cannot be skipped. At each depth, branches to the left
    /// of the path specified by `range_start` can be ignored.
    pub fn can_skip(&self, range_start: &LeafCursor) -> bool {
        match self.path.cmp(&range_start.path) {
            BranchOrdering::Less => true,
            BranchOrdering::Equal => false,
            BranchOrdering::Greater => false,
            BranchOrdering::Ancestor => false,
            BranchOrdering::Descendant => false,
        }
    }
}

impl LeafCursor {
    /// Creates a new empty pseudo-LeafCursor that acts to specify the root of the trie. This is
    /// used as `range_start` in cases where iteration should start from the beginning of the trie.
    pub fn start() -> LeafCursor {
        LeafCursor {
            path: Path::default(),
        }
    }

    /// Returns true if this leaf path should be skipped, given the specified `range_start`
    /// The logic is the same as `can_skip` for a LeafBranch, but includes a case for equal leaves.
    /// Since the cursor returned by iteration points to the last traversed value, the next iteration
    /// must skip that particular leaf.
    pub fn can_skip(&self, range_start: &LeafCursor) -> bool {
        match self.path.cmp(&range_start.path) {
            BranchOrdering::Less | BranchOrdering::Equal => true,
            BranchOrdering::Greater => false,
            BranchOrdering::Ancestor => false,
            BranchOrdering::Descendant => false,
        }
    }
}
