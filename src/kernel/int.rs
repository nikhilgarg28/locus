//! Arbitrary-precision integers: a sign and a `Natural` magnitude. These are
//! the integers of the logic, so nothing overflows and every operation is
//! total: division truncates toward zero as Rust's `/` and `%` do, and
//! division by zero is defined so that `a == (a / b) * b + a % b` always.
//! Part of the trusted base, because evaluation computes with it.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use super::nat::{Natural, ParseNumberError};

/// Zero is never negative, so equal numbers are equal as data. The fields
/// are private and every constructor goes through `from_parts`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Integer {
    negative: bool,
    magnitude: Natural,
}

impl Integer {
    pub fn zero() -> Self {
        Self::default()
    }

    /// The number with this sign and magnitude; the sign of zero is dropped.
    pub fn from_parts(negative: bool, magnitude: Natural) -> Self {
        Self {
            negative: negative && !magnitude.is_zero(),
            magnitude,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.magnitude.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    /// The absolute value.
    pub fn magnitude(&self) -> &Natural {
        &self.magnitude
    }

    pub fn neg(&self) -> Self {
        Self::from_parts(!self.negative, self.magnitude.clone())
    }

    pub fn add(&self, other: &Self) -> Self {
        if self.negative == other.negative {
            return Self::from_parts(self.negative, self.magnitude.add(&other.magnitude));
        }
        // Opposite signs: the difference of the magnitudes, with the sign of
        // the larger. Exactly one of the two subtractions is defined unless
        // the magnitudes are equal, and then both give zero.
        match self.magnitude.checked_sub(&other.magnitude) {
            Some(difference) => Self::from_parts(self.negative, difference),
            None => Self::from_parts(
                other.negative,
                other
                    .magnitude
                    .checked_sub(&self.magnitude)
                    .unwrap_or_default(),
            ),
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.neg())
    }

    pub fn mul(&self, other: &Self) -> Self {
        Self::from_parts(
            self.negative != other.negative,
            self.magnitude.mul(&other.magnitude),
        )
    }

    /// The quotient truncated toward zero; `a / 0` is `0`.
    pub fn div(&self, other: &Self) -> Self {
        self.div_rem(other).0
    }

    /// The remainder of truncating division: it has the sign of `self` and
    /// a magnitude below that of `other`; `a % 0` is `a`.
    pub fn rem(&self, other: &Self) -> Self {
        self.div_rem(other).1
    }

    /// Both at once. Truncation toward zero is division of the magnitudes:
    /// the quotient is negative when the signs differ, and the remainder
    /// takes the sign of the dividend.
    pub fn div_rem(&self, other: &Self) -> (Self, Self) {
        match self.magnitude.div_rem(&other.magnitude) {
            None => (Self::zero(), self.clone()),
            Some((quotient, remainder)) => (
                Self::from_parts(self.negative != other.negative, quotient),
                Self::from_parts(self.negative, remainder),
            ),
        }
    }

    pub fn to_i128(&self) -> Option<i128> {
        let magnitude = self.magnitude.to_u128()?;
        if self.negative {
            0i128.checked_sub_unsigned(magnitude)
        } else {
            i128::try_from(magnitude).ok()
        }
    }

    /// The same number as a natural, or `None` when it is negative.
    pub fn to_natural(&self) -> Option<Natural> {
        (!self.negative).then(|| self.magnitude.clone())
    }
}

impl From<i128> for Integer {
    fn from(value: i128) -> Self {
        Self::from_parts(value < 0, Natural::from_u128(value.unsigned_abs()))
    }
}

impl From<i64> for Integer {
    fn from(value: i64) -> Self {
        Self::from(i128::from(value))
    }
}

impl From<u128> for Integer {
    fn from(value: u128) -> Self {
        Self::from(Natural::from_u128(value))
    }
}

impl From<Natural> for Integer {
    fn from(magnitude: Natural) -> Self {
        Self::from_parts(false, magnitude)
    }
}

/// Negative numbers come first, and among them the larger magnitude is the
/// smaller number.
impl Ord for Integer {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (false, false) => self.magnitude.cmp(&other.magnitude),
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (true, true) => other.magnitude.cmp(&self.magnitude),
        }
    }
}

impl PartialOrd for Integer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// An optional `-` and then decimal digits as `Natural` reads them. `-0` is
/// zero.
impl FromStr for Integer {
    type Err = ParseNumberError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.strip_prefix('-') {
            Some(digits) => Ok(Self::from_parts(true, digits.parse()?)),
            None => Ok(Self::from_parts(false, text.parse()?)),
        }
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.negative {
            f.write_str("-")?;
        }
        self.magnitude.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::Integer;

    #[test]
    fn division_truncates_toward_zero_and_is_total() {
        for (a, b) in [
            (7i64, 2i64),
            (-7, 2),
            (7, -2),
            (-7, -2),
            (6, 3),
            (0, 5),
            (1, 9),
        ] {
            let (quotient, remainder) = Integer::from(a).div_rem(&Integer::from(b));
            assert_eq!(quotient, Integer::from(a / b), "{a} / {b}");
            assert_eq!(remainder, Integer::from(a % b), "{a} % {b}");
        }
        let seven = Integer::from(-7i64);
        assert_eq!(seven.div(&Integer::zero()), Integer::zero());
        assert_eq!(seven.rem(&Integer::zero()), seven);
    }

    #[test]
    fn there_is_one_zero_and_no_overflow() {
        let zero = Integer::zero();
        assert_eq!(zero.neg(), zero);
        assert_eq!("-0".parse(), Ok(zero.clone()));
        assert_eq!(Integer::from(-3i64).mul(&zero), zero);
        assert_eq!(Integer::from(-3i64).add(&Integer::from(3i64)), zero);
        assert_eq!(Integer::from(-3i64).rem(&Integer::from(3i64)), zero);
        assert!(!Integer::from(-1i64).div(&Integer::from(2i64)).is_negative());

        let min = Integer::from(i128::MIN);
        let past = min.div(&Integer::from(-1i64));
        assert_eq!(past.to_i128(), None);
        assert_eq!(past.to_string(), "170141183460469231731687303715884105728");
        assert_eq!(past.neg().to_i128(), Some(i128::MIN));
        assert_eq!(
            Integer::from(u128::MAX).to_natural().unwrap().to_u128(),
            Some(u128::MAX)
        );
        assert_eq!(min.to_natural(), None);
    }
}
