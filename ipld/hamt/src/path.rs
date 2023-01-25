// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cmp::Ordering;

#[derive(Default, PartialEq, Eq, Clone, Debug)]
pub struct Path {
    branches: Vec<u8>,
}

impl PartialOrd for Path {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // compare along the entire path
        for (branch, gate) in self.branches.iter().zip(other.branches.iter()) {
            match branch.cmp(gate) {
                std::cmp::Ordering::Less => {
                    // if the path has a smaller branch at any point, it can definitely be skipped
                    return Some(Ordering::Less);
                }
                std::cmp::Ordering::Greater => {
                    // if the path is larger at any depth, it cannot be skipped
                    return Some(Ordering::Greater);
                }
                std::cmp::Ordering::Equal => {
                    // path is equal to the range start so far, continue checking further branches
                }
            }
        }

        // we explored the entire path and it matched, therefore it is a direct ancestor or the
        // range start itself, and cannot be skipped
        if self.branches.len() > other.branches.len() {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}

impl Ord for Path {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Path {
    // Creates a new path with the extra branch specified at the end
    pub fn create_branch(&self, branch: u8) -> Path {
        let mut new_path = self.clone();
        new_path.branches.push(branch);
        new_path
    }

    // Returns true if the current path can be safely skipped, given the specified `range_start`.
    // Direct ancestors of the range start cannot be skipped. At each depth, branches to the left of
    // the path specified by `range_start` can be ignored.
    pub fn can_skip(&self, range_start: &Path) -> bool {
        self.cmp(range_start) == Ordering::Less
    }

    // Returns the next uncle path
    // FIXME: this may not be a valid path (i.e. when the branch number exceeds the bitfield length,
    // in these cases, unecessary nodes will be explored on the next iteration)
    // FIMXE: when the parent is branch number 255_u8, this will break
    pub fn next_uncle(&self) -> Path {
        if self.branches.len() <= 1 {
            return Path::default();
        }
        let mut new_vec = self.branches.clone();
        new_vec.pop();

        let parent = new_vec.pop().unwrap();
        new_vec.push(parent + 1);
        Path { branches: new_vec }
    }
}
