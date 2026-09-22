//! Arbitrary-precision natural numbers: the magnitudes of `Integer` and the
//! values of the lexer's literals. Addition, checked subtraction, multiplication,
//! division with remainder, comparison, decimal parsing and printing.
//! Part of the trusted base, because evaluation computes with it. Every
//! algorithm is the schoolbook one: clarity is worth more here than speed.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// Little-endian base-2^32 limbs with no trailing zero limb, so equal
/// numbers are equal as data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Natural {
    limbs: Vec<u32>,
}

impl Natural {
    pub fn zero() -> Self {
        Self::default()
    }

    /// Drops trailing zero limbs, which restores the invariant.
    fn from_limbs(mut limbs: Vec<u32>) -> Self {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Self { limbs }
    }

    pub fn from_u128(value: u128) -> Self {
        Self::from_limbs(vec![
            value as u32,
            (value >> 32) as u32,
            (value >> 64) as u32,
            (value >> 96) as u32,
        ])
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub fn succ(&self) -> Self {
        self.add(&Self::from(1))
    }

    pub fn add(&self, other: &Self) -> Self {
        let mut limbs = Vec::with_capacity(self.limbs.len().max(other.limbs.len()) + 1);
        let mut carry = 0u64;
        for index in 0..self.limbs.len().max(other.limbs.len()) {
            let left = u64::from(self.limbs.get(index).copied().unwrap_or(0));
            let right = u64::from(other.limbs.get(index).copied().unwrap_or(0));
            let sum = left + right + carry;
            limbs.push(sum as u32);
            carry = sum >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        Self { limbs }
    }

    /// `self - other`, or `None` when that would be negative.
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        if other.limbs.len() > self.limbs.len() {
            return None;
        }
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow = 0u64;
        for (index, limb) in self.limbs.iter().enumerate() {
            let left = u64::from(*limb);
            let right = u64::from(other.limbs.get(index).copied().unwrap_or(0)) + borrow;
            if left >= right {
                limbs.push((left - right) as u32);
                borrow = 0;
            } else {
                limbs.push((left + (1 << 32) - right) as u32);
                borrow = 1;
            }
        }
        // A borrow out of the top limb means `other` was the larger.
        (borrow == 0).then(|| Self::from_limbs(limbs))
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (i, left) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, right) in other.limbs.iter().enumerate() {
                // At most (2^32 - 1)^2 + 2 * (2^32 - 1) = 2^64 - 1: no overflow.
                let current =
                    u64::from(limbs[i + j]) + u64::from(*left) * u64::from(*right) + carry;
                limbs[i + j] = current as u32;
                carry = current >> 32;
            }
            // No row up to this one has written this limb: it is still zero.
            limbs[i + other.limbs.len()] = carry as u32;
        }
        Self::from_limbs(limbs)
    }

    /// The quotient and remainder, or `None` when `divisor` is zero.
    /// Long division in base 2: bring down one bit of `self` at a time,
    /// keeping `remainder < divisor` by subtracting whenever that is possible.
    pub fn div_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        if divisor.is_zero() {
            return None;
        }
        let mut quotient = vec![0u32; self.limbs.len()];
        let mut remainder = Self::zero();
        for index in (0..self.bit_length()).rev() {
            remainder.double_and_add(self.bit(index));
            if let Some(rest) = remainder.checked_sub(divisor) {
                remainder = rest;
                quotient[index / 32] |= 1 << (index % 32);
            }
        }
        Some((Self::from_limbs(quotient), remainder))
    }

    /// The number of bits needed to write the number; zero needs none.
    pub fn bit_length(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(top) => self.limbs.len() * 32 - top.leading_zeros() as usize,
        }
    }

    /// Bit `index`, counting from the least significant; zero past the end.
    fn bit(&self, index: usize) -> bool {
        self.limbs
            .get(index / 32)
            .is_some_and(|limb| (limb >> (index % 32)) & 1 == 1)
    }

    /// Replaces `self` by `2 * self + bit`.
    fn double_and_add(&mut self, bit: bool) {
        let mut carry = u32::from(bit);
        for limb in &mut self.limbs {
            let next = *limb >> 31;
            *limb = (*limb << 1) | carry;
            carry = next;
        }
        if carry > 0 {
            self.limbs.push(carry);
        }
    }

    /// The number modulo 256.
    pub fn low_byte(&self) -> u8 {
        self.limbs.first().map_or(0, |limb| *limb as u8)
    }

    pub fn to_u64(&self) -> Option<u64> {
        match self.limbs.as_slice() {
            [] => Some(0),
            [low] => Some(u64::from(*low)),
            [low, high] => Some(u64::from(*low) | (u64::from(*high) << 32)),
            _ => None,
        }
    }

    pub fn to_u128(&self) -> Option<u128> {
        if self.limbs.len() > 4 {
            return None;
        }
        let mut value = 0u128;
        for limb in self.limbs.iter().rev() {
            value = (value << 32) | u128::from(*limb);
        }
        Some(value)
    }

    /// Divides in place by a small number and returns the remainder.
    /// The divisor is never zero: the only caller passes ten.
    fn div_rem_small(&mut self, divisor: u32) -> u32 {
        let mut remainder = 0u64;
        for limb in self.limbs.iter_mut().rev() {
            let current = (remainder << 32) | u64::from(*limb);
            *limb = (current / u64::from(divisor)) as u32;
            remainder = current % u64::from(divisor);
        }
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        remainder as u32
    }
}

impl From<u64> for Natural {
    fn from(value: u64) -> Self {
        Self::from_limbs(vec![value as u32, (value >> 32) as u32])
    }
}

/// With no trailing zero limb the longer number is the larger, and numbers
/// of equal length compare limb by limb from the most significant end.
impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}

impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The text was not a number written in decimal digits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseNumberError;

impl fmt::Display for ParseNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected decimal digits")
    }
}

impl std::error::Error for ParseNumberError {}

/// One or more ASCII decimal digits, as many as the text holds. Leading
/// zeros are allowed; signs and separators are not.
impl FromStr for Natural {
    type Err = ParseNumberError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(ParseNumberError);
        }
        let ten = Self::from(10);
        let mut value = Self::zero();
        for byte in text.bytes() {
            if !byte.is_ascii_digit() {
                return Err(ParseNumberError);
            }
            value = value.mul(&ten).add(&Self::from(u64::from(byte - b'0')));
        }
        Ok(value)
    }
}

impl fmt::Display for Natural {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }
        let mut rest = self.clone();
        let mut digits = Vec::new();
        while !rest.is_zero() {
            digits.push(char::from(b'0' + rest.div_rem_small(10) as u8));
        }
        f.write_str(&digits.iter().rev().collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::Natural;

    #[test]
    fn arithmetic_agrees_with_machine_integers_and_continues_past_them() {
        for (a, b) in [
            (0u64, 0u64),
            (1, 2),
            (u32::MAX.into(), 1),
            (1 << 40, 1 << 40),
        ] {
            let sum = Natural::from(a).add(&Natural::from(b));
            assert_eq!(sum, Natural::from(a + b));
            assert_eq!(sum.to_u64(), Some(a + b));
            assert_eq!(sum.low_byte(), ((a + b) % 256) as u8);
        }
        let past = Natural::from(u64::MAX).succ();
        assert_eq!(past.to_u64(), None);
        assert_eq!(past.to_string(), "18446744073709551616");
        assert_eq!(past.low_byte(), 0);
        assert_eq!(past.add(&past).to_string(), "36893488147419103232");
        assert_eq!(Natural::zero().to_string(), "0");
        assert_eq!(Natural::from(1234567890123).to_string(), "1234567890123");
    }

    #[test]
    fn subtraction_multiplication_and_division_cross_limb_boundaries() {
        let two_64 = Natural::from(u64::MAX).succ();
        let two_128 = two_64.mul(&two_64);
        assert_eq!(
            two_128.to_string(),
            "340282366920938463463374607431768211456"
        );
        assert_eq!(two_128.to_u128(), None);
        assert_eq!(two_128.bit_length(), 129);

        let max = two_128.checked_sub(&Natural::from(1)).unwrap();
        assert_eq!(max, Natural::from_u128(u128::MAX));
        assert_eq!(max.to_u128(), Some(u128::MAX));
        assert_eq!(max.checked_sub(&two_128), None);
        assert_eq!(max.checked_sub(&max), Some(Natural::zero()));
        assert!(max < two_128 && two_64 < max);

        let (quotient, remainder) = max.div_rem(&two_64).unwrap();
        assert_eq!(quotient, Natural::from(u64::MAX));
        assert_eq!(remainder, Natural::from(u64::MAX));
        assert_eq!(max.div_rem(&Natural::zero()), None);

        assert_eq!("000".parse(), Ok(Natural::zero()));
        assert_eq!(two_128.to_string().parse(), Ok(two_128));
        for text in ["", "-1", "+1", "1_000", "12a", " 1", "١"] {
            assert!(text.parse::<Natural>().is_err(), "{text:?}");
        }
    }
}
