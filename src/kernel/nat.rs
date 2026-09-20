//! Arbitrary-precision natural numbers for `Nat` literals. Only what literal
//! evaluation needs: successor, addition, the low byte, and printing.
//! Part of the trusted base, because `literal` evaluates with it.

use std::fmt;

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

    /// Divides in place by a small number and returns the remainder.
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
        let mut limbs = vec![value as u32, (value >> 32) as u32];
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Self { limbs }
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
}
