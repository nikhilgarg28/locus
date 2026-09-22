//! Rational numbers, for the point that back-substitution through the
//! elimination produces. Only what that needs: comparison, the floor and
//! the ceiling, and division of an integer by an integer.

use std::cmp::Ordering;

use crate::kernel::Integer;

use super::form::gcd;

/// `numerator / denominator` in lowest terms, with a positive denominator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Rational {
    numerator: Integer,
    denominator: Integer,
}

impl Rational {
    pub(super) fn integer(value: Integer) -> Self {
        Self {
            numerator: value,
            denominator: Integer::from(1i64),
        }
    }

    /// `numerator / denominator`; the denominator must not be zero.
    pub(super) fn new(numerator: Integer, denominator: Integer) -> Self {
        assert!(
            !denominator.is_zero(),
            "a rational has a non-zero denominator"
        );
        let divisor = gcd(&numerator, &denominator);
        let (mut numerator, mut denominator) = (numerator.div(&divisor), denominator.div(&divisor));
        if denominator.is_negative() {
            numerator = numerator.neg();
            denominator = denominator.neg();
        }
        Self {
            numerator,
            denominator,
        }
    }

    /// The integer the rational is, if it is one.
    pub(super) fn as_integer(&self) -> Option<&Integer> {
        (self.denominator == Integer::from(1i64)).then_some(&self.numerator)
    }

    /// The greatest integer not above the rational.
    pub(super) fn floor(&self) -> Integer {
        let (quotient, remainder) = self.numerator.div_rem(&self.denominator);
        if remainder.is_negative() {
            quotient.sub(&Integer::from(1i64))
        } else {
            quotient
        }
    }

    /// The least integer not below the rational.
    pub(super) fn ceiling(&self) -> Integer {
        let floor = self.floor();
        if self.as_integer().is_some() {
            floor
        } else {
            floor.add(&Integer::from(1i64))
        }
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        // Both denominators are positive, so cross-multiplying keeps the
        // order.
        self.numerator
            .mul(&other.denominator)
            .cmp(&other.numerator.mul(&self.denominator))
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
