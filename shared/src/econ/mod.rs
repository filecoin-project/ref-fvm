use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Signed, Zero};
use serde::{Deserialize, Serialize, Serializer};

use crate::bigint::bigint_ser;

/// A quantity of native tokens.
/// A token amount is an integer, but has a human interpretation as a value with
/// 18 decimal places.
/// This is a new-type in order to prevent accidental conversion from other BigInts.
/// From/Into BigInt is missing by design.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TokenAmount {
    atto: BigInt,
}

// This type doesn't implement all the numeric traits (Num, Signed, etc),
// opting for a minimal useful set. Others can be added if needed.
impl TokenAmount {
    /// The logical number of decimal places of a token unit.
    pub const DECIMALS: usize = 18;

    /// The logical precision of a token unit.
    pub const PRECISION: u64 = 10u64.pow(Self::DECIMALS as u32);

    /// Creates a token amount from a quantity of indivisible units  (10^-18 whole units).
    pub fn from_atto(atto: impl Into<BigInt>) -> Self {
        Self { atto: atto.into() }
    }

    /// Creates a token amount from a quantity of whole units (10^18 indivisible units).
    pub fn from_whole(tokens: i64) -> Self {
        Self::from_atto((tokens as i128) * (Self::PRECISION as i128))
    }

    /// Returns the quantity of indivisible units.
    pub fn atto(&self) -> &BigInt {
        &self.atto
    }

    pub fn is_zero(&self) -> bool {
        self.atto.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.atto.is_positive()
    }

    pub fn is_negative(&self) -> bool {
        self.atto.is_negative()
    }
}

impl Zero for TokenAmount {
    #[inline]
    fn zero() -> Self {
        Self {
            atto: BigInt::zero(),
        }
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.atto.is_zero()
    }
}

impl PartialOrd for TokenAmount {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.atto.partial_cmp(&other.atto)
    }
}

impl Ord for TokenAmount {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.atto.cmp(&other.atto)
    }
}

impl Default for TokenAmount {
    #[inline]
    fn default() -> TokenAmount {
        TokenAmount::zero()
    }
}

impl fmt::Debug for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TokenAmount({})", self)
    }
}

/// Displays a token amount as a decimal in human units.
/// To avoid any confusion over whether the value is in human-scale or indivisible units,
/// the display always includes a decimal point.
impl fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Implementation based on the bigdecimal library.
        let (q, r) = self.atto.div_rem(&BigInt::from(Self::PRECISION));
        let before_decimal = q.abs().to_str_radix(10);
        let after_decimal = if r.is_zero() {
            "0".to_string()
        } else {
            let fraction_str = r.to_str_radix(10);
            let render = "0".repeat(Self::DECIMALS - fraction_str.len()) + fraction_str.as_str();
            render.trim_end_matches('0').to_string()
        };

        // Alter precision after the decimal point
        let after_decimal = if let Some(precision) = f.precision() {
            let len = after_decimal.len();
            if len < precision {
                after_decimal + "0".repeat(precision - len).as_str()
            } else {
                after_decimal[0..precision].to_string()
            }
        } else {
            after_decimal
        };

        // Always show the decimal point, even with ".0".
        let complete_without_sign = before_decimal + "." + after_decimal.as_str();
        // Padding works even though we have a decimal point.
        f.pad_integral(!q.is_negative(), "", &complete_without_sign)
    }
}

impl Neg for TokenAmount {
    type Output = TokenAmount;

    #[inline]
    fn neg(self) -> TokenAmount {
        TokenAmount { atto: -self.atto }
    }
}

impl<'a> Neg for &'a TokenAmount {
    type Output = TokenAmount;

    #[inline]
    fn neg(self) -> TokenAmount {
        TokenAmount {
            atto: (&self.atto).neg(),
        }
    }
}

// Implements Add for all combinations of value/reference receiver and parameter.
// (Pattern copied from BigInt multiplication).
macro_rules! impl_add {
    ($(impl<$($a:lifetime),*> Add<$Other:ty> for $Self:ty;)*) => {$(
        impl<$($a),*> Add<$Other> for $Self {
            type Output = TokenAmount;

            #[inline]
            fn add(self, other: $Other) -> TokenAmount {
                // automatically match value/ref
                let TokenAmount { atto: x, .. } = self;
                let TokenAmount { atto: y, .. } = other;
                TokenAmount {atto: x + y}
            }
        }
    )*}
}
impl_add! {
    impl<> Add<TokenAmount> for TokenAmount;
    impl<'b> Add<&'b TokenAmount> for TokenAmount;
    impl<'a> Add<TokenAmount> for &'a TokenAmount;
    impl<'a, 'b> Add<&'b TokenAmount> for &'a TokenAmount;
}

// Implements Sub for all combinations of value/reference receiver and parameter.
macro_rules! impl_sub {
    ($(impl<$($a:lifetime),*> Sub<$Other:ty> for $Self:ty;)*) => {$(
        impl<$($a),*> Sub<$Other> for $Self {
            type Output = TokenAmount;

            #[inline]
            fn sub(self, other: $Other) -> TokenAmount {
                // automatically match value/ref
                let TokenAmount { atto: x, .. } = self;
                let TokenAmount { atto: y, .. } = other;
                TokenAmount {atto: x - y}
            }
        }
    )*}
}
impl_sub! {
    impl<> Sub<TokenAmount> for TokenAmount;
    impl<'b> Sub<&'b TokenAmount> for TokenAmount;
    impl<'a> Sub<TokenAmount> for &'a TokenAmount;
    impl<'a, 'b> Sub<&'b TokenAmount> for &'a TokenAmount;
}

// Implements Mul for all combinations of value/reference receiver and 32/64-bit value.
macro_rules! impl_mul {
    ($(impl<$($a:lifetime),*> Mul<$Other:ty> for $Self:ty;)*) => {$(
        impl<$($a),*> Mul<$Other> for $Self {
            type Output = TokenAmount;

            #[inline]
            fn mul(self, other: $Other) -> TokenAmount {
                // automatically match value/ref
                let TokenAmount { atto: x, .. } = self;
                // let TokenAmount { atto: y, .. } = other;
                TokenAmount {atto: x * other}
            }
        }
    )*}
}
impl_mul! {
    impl<> Mul<u32> for TokenAmount;
    impl<'a> Mul<u32> for &'a TokenAmount;
    impl<> Mul<i32> for TokenAmount;
    impl<'a> Mul<i32> for &'a TokenAmount;
    impl<> Mul<u64> for TokenAmount;
    impl<'a> Mul<u64> for &'a TokenAmount;
    impl<> Mul<i64> for TokenAmount;
    impl<'a> Mul<i64> for &'a TokenAmount;
}

// Only a single div/rem method is implemented, rather than the full Div and Rem traits.
// Division isn't a common operation with money-like units, and deserves to be treated carefully.
impl TokenAmount {
    #[inline]
    pub fn div_rem(&self, other: &TokenAmount) -> (TokenAmount, TokenAmount) {
        let (q, r) = self.atto.div_rem(&other.atto);
        (TokenAmount { atto: q }, TokenAmount { atto: r })
    }
}

// Serialisation

impl Serialize for TokenAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bigint_ser::serialize(&self.atto, serializer)
    }
}

impl<'de> Deserialize<'de> for TokenAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        bigint_ser::deserialize(deserializer).map(|v| TokenAmount { atto: v })
    }
}

#[cfg(test)]
mod test {
    use num_traits::Zero;

    use crate::TokenAmount;

    fn basic(expected: &str, t: TokenAmount) {
        assert_eq!(expected, format!("{}", t));
    }

    #[test]
    fn display_basic() {
        basic("0.0", TokenAmount::zero());
        basic("0.000000000000000001", TokenAmount::from_atto(1));
        basic("0.000000000000001", TokenAmount::from_atto(1000));
        basic(
            "0.1234",
            TokenAmount::from_atto(123_400_000_000_000_000_u64),
        );
        basic(
            "0.10101",
            TokenAmount::from_atto(101_010_000_000_000_000_u64),
        );
        basic("1.0", TokenAmount::from_whole(1));
        basic(
            "1.0",
            TokenAmount::from_atto(1_000_000_000_000_000_000_u128),
        );
        basic(
            "1.1",
            TokenAmount::from_atto(1_100_000_000_000_000_000_u128),
        );
        basic(
            "1.000000000000000001",
            TokenAmount::from_atto(1_000_000_000_000_000_001_u128),
        );
        basic(
            "1234.000000000123456789",
            TokenAmount::from_whole(1234) + TokenAmount::from_atto(123_456_789_u64),
        );
    }

    #[test]
    fn display_precision() {
        assert_eq!("0.0", format!("{:.1}", TokenAmount::zero()));
        assert_eq!("0.000", format!("{:.3}", TokenAmount::zero()));
        assert_eq!("0.000", format!("{:.3}", TokenAmount::from_atto(1))); // Truncated.
        assert_eq!(
            "0.123",
            format!("{:.3}", TokenAmount::from_atto(123_456_789_000_000_000_u64)) // Truncated.
        );
        assert_eq!(
            "0.123456789000",
            format!(
                "{:.12}",
                TokenAmount::from_atto(123_456_789_000_000_000_u64)
            )
        );
    }

    #[test]
    fn display_padding() {
        assert_eq!("0.0", format!("{:01}", TokenAmount::zero()));
        assert_eq!("0.0", format!("{:03}", TokenAmount::zero()));
        assert_eq!("000.0", format!("{:05}", TokenAmount::zero()));
        assert_eq!(
            "0.123",
            format!(
                "{:01.3}",
                TokenAmount::from_atto(123_456_789_000_000_000_u64)
            )
        );
        assert_eq!(
            "00.123",
            format!(
                "{:06.3}",
                TokenAmount::from_atto(123_456_789_000_000_000_u64)
            )
        );
    }
}
