//! This module contains types exchanged at the syscall layer between actors
//! (usually through the SDK) and the FVM.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Sub};

use num_bigint::Sign;

use crate::sys::TokenAmountError::{Overflow, Underflow};

pub mod out;
pub mod tokenamount_ser;

pub type BlockId = u32;
pub type Codec = u64;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TokenAmount {
    // DO NOT reorder these fields. The layout is equivalent to u128 on a big-endian system, and
    // optimizes well.
    lo: u64,
    hi: u64,
}

#[derive(Debug)]
pub enum TokenAmountError {
    Overflow,
    Underflow,
}

impl Add<Self> for TokenAmount {
    type Output = Result<TokenAmount, TokenAmountError>;

    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).ok_or(Overflow)
    }
}

impl Sub<Self> for TokenAmount {
    type Output = Result<TokenAmount, TokenAmountError>;

    fn sub(self, rhs: Self) -> Self::Output {
        self.checked_add(rhs).ok_or(Underflow)
    }
}
//
// impl Mul<u64> for TokenAmount {
//     type Output = TokenAmount;
//
//     fn mul(self, rhs: u64) -> Self::Output {
//         let eta: crate::econ::TokenAmount = self.into();
//         crate::econ::TokenAmount::from(eta * rhs)
//             .try_into()
//             .unwrap()
//     }
// }
//

impl Mul<i64> for TokenAmount {
    type Output = Result<TokenAmount, TokenAmountError>;

    fn mul(self, rhs: i64) -> Self::Output {
        if rhs < 0 {
            return Err(Underflow);
        }

        match u128::from(self).checked_mul(rhs as u128) {
            None => Err(Overflow),
            Some(v) => Ok(v.into()),
        }
    }
}

//
// impl Mul<u64> for &TokenAmount {
//     type Output = TokenAmount;
//
//     fn mul(self, rhs: u64) -> Self::Output {
//         *self * rhs
//     }
// }
//
// impl Mul<i64> for &TokenAmount {
//     type Output = TokenAmount;
//
//     fn mul(self, rhs: i64) -> Self::Output {
//         *self * rhs
//     }
// }

impl From<TokenAmount> for u128 {
    #[inline]
    fn from(v: TokenAmount) -> Self {
        (v.hi as u128) << u64::BITS | (v.lo as u128)
    }
}

impl From<&TokenAmount> for u128 {
    #[inline]
    fn from(v: &TokenAmount) -> Self {
        (v.hi as u128) << u64::BITS | (v.lo as u128)
    }
}

impl From<TokenAmount> for crate::econ::TokenAmount {
    fn from(v: TokenAmount) -> Self {
        crate::econ::TokenAmount::from(u128::from(v))
    }
}

impl From<&TokenAmount> for crate::econ::TokenAmount {
    fn from(v: &TokenAmount) -> Self {
        crate::econ::TokenAmount::from(u128::from(v))
    }
}

impl From<u32> for TokenAmount {
    fn from(v: u32) -> Self {
        TokenAmount {
            hi: 0,
            lo: v as u64,
        }
    }
}

impl From<u64> for TokenAmount {
    fn from(v: u64) -> Self {
        TokenAmount { hi: 0, lo: v }
    }
}

impl From<u128> for TokenAmount {
    fn from(v: u128) -> Self {
        TokenAmount {
            hi: (v >> u64::BITS) as u64,
            lo: v as u64,
        }
    }
}

// review: these next 2 methods should fail on negative input, unless the map to u128 does that for you
impl TryFrom<crate::econ::TokenAmount> for TokenAmount {
    type Error = TokenAmountError;
    fn try_from(v: crate::econ::TokenAmount) -> Result<Self, Self::Error> {
        match v.sign() {
            Sign::Minus => Err(Underflow),
            Sign::NoSign => Ok(TokenAmount::zero()),
            Sign::Plus => match u128::try_from(v) {
                Ok(v128) => Ok(v128.into()),
                Err(_) => Err(Overflow),
            },
        }
    }
}

impl<'a> TryFrom<&'a crate::econ::TokenAmount> for TokenAmount {
    type Error = TokenAmountError;
    fn try_from(v: &'a crate::econ::TokenAmount) -> Result<Self, Self::Error> {
        match v.sign() {
            Sign::Minus => Err(Underflow),
            Sign::NoSign => Ok(TokenAmount::zero()),
            Sign::Plus => match u128::try_from(v) {
                Ok(v128) => Ok(v128.into()),
                Err(_) => Err(Overflow),
            },
        }
    }
}

impl fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", crate::econ::TokenAmount::from(self))
    }
}

impl PartialEq<Self> for TokenAmount {
    fn eq(&self, other: &Self) -> bool {
        self.hi == other.hi && self.lo == other.lo
    }
}

impl Eq for TokenAmount {}

impl Hash for TokenAmount {
    fn hash<H: Hasher>(&self, state: &mut H) {
        u128::from(self).hash(state)
    }
}

impl PartialOrd<Self> for TokenAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        u128::from(self).partial_cmp(&u128::from(other))
    }
}

impl Ord for TokenAmount {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Default for TokenAmount {
    #[inline]
    fn default() -> TokenAmount {
        TokenAmount::zero()
    }
}

impl TokenAmount {
    pub fn checked_sub(&self, rhs: TokenAmount) -> Option<TokenAmount> {
        match u128::from(self).checked_sub(u128::from(rhs)) {
            None => None,
            Some(v) => Some(v.into()),
        }
    }

    pub fn checked_add(&self, rhs: TokenAmount) -> Option<TokenAmount> {
        match u128::from(self).checked_add(u128::from(rhs)) {
            None => None,
            Some(v) => Some(v.into()),
        }
    }

    // should they have inline directives?
    pub fn zero() -> TokenAmount {
        TokenAmount { hi: 0, lo: 0 }
    }

    pub fn is_zero(&self) -> bool {
        self.hi == 0 && self.lo == 0
    }

    pub fn high_low(&self) -> (u64, u64) {
        (self.hi, self.lo)
    }
}
