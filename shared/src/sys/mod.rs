//! This module contains types exchanged at the syscall layer between actors
//! (usually through the SDK) and the FVM.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Sub};

use num_traits::Zero;

pub mod out;
pub mod tokenamount_ser;

pub type BlockId = u32;
pub type Codec = u64;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TokenAmount {
    pub lo: u64,
    pub hi: u64,
}

impl TokenAmount {
    pub fn checked_sub(&self, v: &TokenAmount) -> Option<TokenAmount> {
        if self < v {
            return None;
        }

        crate::econ::TokenAmount::from(self)
            .checked_sub(&crate::econ::TokenAmount::from(v))
            .unwrap()
            .try_into()
            .ok()
    }

    pub fn checked_add(&self, v: &TokenAmount) -> Option<TokenAmount> {
        crate::econ::TokenAmount::from(self)
            .checked_add(&crate::econ::TokenAmount::from(v))
            .unwrap()
            .try_into()
            .ok()
    }
}

impl From<TokenAmount> for crate::econ::TokenAmount {
    fn from(v: TokenAmount) -> Self {
        crate::econ::TokenAmount::from(v.hi) << 64 | crate::econ::TokenAmount::from(v.lo)
    }
}

impl From<&TokenAmount> for crate::econ::TokenAmount {
    fn from(v: &TokenAmount) -> Self {
        crate::econ::TokenAmount::from(v.hi) << 64 | crate::econ::TokenAmount::from(v.lo)
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
    type Error = <crate::econ::TokenAmount as TryInto<u128>>::Error;
    fn try_from(v: crate::econ::TokenAmount) -> Result<Self, Self::Error> {
        v.try_into().map(|v: u128| Self {
            hi: (v >> u64::BITS) as u64,
            lo: v as u64,
        })
    }
}

impl<'a> TryFrom<&'a crate::econ::TokenAmount> for TokenAmount {
    type Error = <&'a crate::econ::TokenAmount as TryInto<u128>>::Error;
    fn try_from(v: &'a crate::econ::TokenAmount) -> Result<Self, Self::Error> {
        v.try_into().map(|v: u128| Self {
            hi: (v >> u64::BITS) as u64,
            lo: v as u64,
        })
    }
}

impl fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", crate::econ::TokenAmount::from(self))
    }
}

impl Add<Self> for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        crate::econ::TokenAmount::from(self)
            .add(&crate::econ::TokenAmount::from(rhs))
            .try_into()
            .unwrap()
    }
}

impl Sub<Self> for TokenAmount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        // panic if rhs > self?
        crate::econ::TokenAmount::from(self)
            .sub(&crate::econ::TokenAmount::from(rhs))
            .try_into()
            .unwrap()
    }
}

impl Zero for TokenAmount {
    #[inline]
    fn zero() -> TokenAmount {
        TokenAmount { hi: 0, lo: 0 }
    }

    #[inline]
    fn set_zero(&mut self) {
        self.hi = 0;
        self.lo = 0;
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.hi == 0 && self.lo == 0
    }
}

impl Default for TokenAmount {
    #[inline]
    fn default() -> TokenAmount {
        Zero::zero()
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
        self.lo.hash(state);
        if self.hi != 0 {
            self.hi.hash(state);
        }
    }
}

impl Mul<u64> for TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: u64) -> Self::Output {
        let eta: crate::econ::TokenAmount = self.into();
        crate::econ::TokenAmount::from(eta * rhs)
            .try_into()
            .unwrap()
    }
}

impl Mul<i64> for TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: i64) -> Self::Output {
        if rhs < 0 {
            // panic?
        }

        self * rhs as u64
    }
}

impl Mul<u64> for &TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: u64) -> Self::Output {
        *self * rhs
    }
}

impl Mul<i64> for &TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: i64) -> Self::Output {
        *self * rhs
    }
}

impl PartialOrd<Self> for TokenAmount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(match self.hi.cmp(&other.hi) {
            Ordering::Less => Ordering::Less,
            Ordering::Equal => self.lo.cmp(&other.lo),
            Ordering::Greater => Ordering::Greater,
        })
    }
}

impl Ord for TokenAmount {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
