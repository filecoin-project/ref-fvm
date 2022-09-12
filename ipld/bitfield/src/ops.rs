use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Sub, SubAssign};

use crate::{BitField, RangeIterator};

// We implement operations both by reference and by-value. The by-value versions can sometimes let
// us avoid a clone, but we implement all by-value variants for symmetry.

/*********/
/*  Or   */
/*********/

impl BitOr<&BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn bitor(self, rhs: &BitField) -> Self::Output {
        if self.is_trivially_empty() {
            rhs.clone()
        } else if rhs.is_trivially_empty() {
            self.clone()
        } else {
            BitField::from_ranges(self.ranges().union(rhs.ranges()))
        }
    }
}

impl BitOr<BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn bitor(self, rhs: BitField) -> Self::Output {
        if self.is_trivially_empty() {
            rhs
        } else if rhs.is_trivially_empty() {
            self
        } else {
            BitField::from_ranges(self.ranges().union(rhs.ranges()))
        }
    }
}

impl BitOr<BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn bitor(self, rhs: BitField) -> Self::Output {
        if self.is_trivially_empty() {
            rhs
        } else if rhs.is_trivially_empty() {
            self.clone()
        } else {
            BitField::from_ranges(self.ranges().union(rhs.ranges()))
        }
    }
}

impl BitOr<&BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn bitor(self, rhs: &BitField) -> Self::Output {
        rhs | self
    }
}

impl BitOrAssign<&BitField> for BitField {
    #[inline]
    fn bitor_assign(&mut self, rhs: &BitField) {
        // Can avoid clones/copies in some cases.
        *self = std::mem::take(self) | rhs;
    }
}

impl BitOrAssign<BitField> for BitField {
    #[inline]
    fn bitor_assign(&mut self, rhs: BitField) {
        // Can avoid clones/copies in some cases.
        *self = std::mem::take(self) | rhs;
    }
}

/*********/
/*  And  */
/*********/

impl BitAnd<&BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn bitand(self, rhs: &BitField) -> Self::Output {
        if self.is_trivially_empty() || rhs.is_trivially_empty() {
            BitField::new()
        } else {
            BitField::from_ranges(self.ranges().intersection(rhs.ranges()))
        }
    }
}

impl BitAnd<BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn bitand(self, rhs: BitField) -> Self::Output {
        // Nothing to optimize.
        &self & &rhs
    }
}

impl BitAnd<BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn bitand(self, rhs: BitField) -> Self::Output {
        // Nothing to optimize.
        self & &rhs
    }
}

impl BitAnd<&BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn bitand(self, rhs: &BitField) -> Self::Output {
        // Nothing to optimize.
        &self & rhs
    }
}

impl BitAndAssign<&BitField> for BitField {
    #[inline]
    fn bitand_assign(&mut self, rhs: &BitField) {
        // Nothing to optimize.
        *self = &*self & rhs;
    }
}

impl BitAndAssign<BitField> for BitField {
    #[inline]
    fn bitand_assign(&mut self, rhs: BitField) {
        // Nothing to optimize.
        *self = &*self & &rhs;
    }
}

/*********/
/*  Sub  */
/*********/

impl Sub<&BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn sub(self, rhs: &BitField) -> Self::Output {
        if self.is_trivially_empty() || rhs.is_trivially_empty() {
            self.clone()
        } else {
            BitField::from_ranges(self.ranges().difference(rhs.ranges()))
        }
    }
}

impl Sub<BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn sub(self, rhs: BitField) -> Self::Output {
        // Delegates to value - ref
        self - &rhs
    }
}

impl Sub<&BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn sub(self, rhs: &BitField) -> Self::Output {
        // Like ref - ref, but avoids a clone.
        if self.is_trivially_empty() || rhs.is_trivially_empty() {
            self
        } else {
            BitField::from_ranges(self.ranges().difference(rhs.ranges()))
        }
    }
}

impl Sub<BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn sub(self, rhs: BitField) -> Self::Output {
        // Nothing to optimize.
        self - &rhs
    }
}

impl SubAssign<&BitField> for BitField {
    #[inline]
    fn sub_assign(&mut self, rhs: &BitField) {
        // Delegates to value - ref
        *self = std::mem::take(self) - rhs;
    }
}

impl SubAssign<BitField> for BitField {
    #[inline]
    fn sub_assign(&mut self, rhs: BitField) {
        // Delegates to value - ref
        *self = std::mem::take(self) - rhs;
    }
}

/*********/
/*  XOR  */
/*********/

impl BitXor<&BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn bitxor(self, rhs: &BitField) -> Self::Output {
        if self.is_trivially_empty() {
            rhs.clone()
        } else if rhs.is_trivially_empty() {
            self.clone()
        } else {
            BitField::from_ranges(self.ranges().symmetric_difference(rhs.ranges()))
        }
    }
}

impl BitXor<BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn bitxor(self, rhs: BitField) -> Self::Output {
        if self.is_trivially_empty() {
            rhs
        } else if rhs.is_trivially_empty() {
            self
        } else {
            BitField::from_ranges(self.ranges().symmetric_difference(rhs.ranges()))
        }
    }
}

impl BitXor<BitField> for &BitField {
    type Output = BitField;

    #[inline]
    fn bitxor(self, rhs: BitField) -> Self::Output {
        if self.is_trivially_empty() {
            rhs
        } else if rhs.is_trivially_empty() {
            self.clone()
        } else {
            BitField::from_ranges(self.ranges().symmetric_difference(rhs.ranges()))
        }
    }
}

impl BitXor<&BitField> for BitField {
    type Output = BitField;

    #[inline]
    fn bitxor(self, rhs: &BitField) -> Self::Output {
        rhs ^ self
    }
}

impl BitXorAssign<&BitField> for BitField {
    #[inline]
    fn bitxor_assign(&mut self, rhs: &BitField) {
        *self = std::mem::take(self) ^ rhs;
    }
}

impl BitXorAssign<BitField> for BitField {
    #[inline]
    fn bitxor_assign(&mut self, rhs: BitField) {
        *self = std::mem::take(self) ^ rhs;
    }
}
