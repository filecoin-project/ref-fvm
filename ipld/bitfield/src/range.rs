// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::ops::Range;

/// A helper trait to get the size of ranges because Range.len doesn't work for ranges of u64.
pub(crate) trait RangeSize {
    type Idx;

    /// Returns the size of the range or 0 of empty.
    fn size(&self) -> Self::Idx;
}

impl RangeSize for Range<u64> {
    type Idx = u64;

    fn size(&self) -> Self::Idx {
        if self.end <= self.start {
            0
        } else {
            self.end - self.start
        }
    }
}

impl RangeSize for Range<u32> {
    type Idx = u32;

    fn size(&self) -> Self::Idx {
        if self.end <= self.start {
            0
        } else {
            self.end - self.start
        }
    }
}
