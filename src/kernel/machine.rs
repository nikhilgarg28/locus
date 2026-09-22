//! The table of machine integer types, `u8` to `u64` and `i8` to `i64`, and
//! reduction into the range of each. This is the one place the kernel knows
//! the width and signedness of a machine type: the primitives `view`,
//! `wrap`, and `cast`, the axioms about them, and native evaluation all read
//! it from here. Part of the trusted base, because evaluation computes
//! `wrap` with it.

use super::int::Integer;

/// A machine integer type. `Type::U8` is the kernel's spelling of the first
/// entry; the other seven are `Type::Machine(_)`. See `Type::machine`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MachineInt {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
}

impl MachineInt {
    /// Every machine integer type, unsigned first, narrowest first.
    pub const ALL: [MachineInt; 8] = [
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
    ];

    pub fn bits(self) -> u32 {
        match self {
            Self::U8 | Self::I8 => 8,
            Self::U16 | Self::I16 => 16,
            Self::U32 | Self::I32 => 32,
            Self::U64 | Self::I64 => 64,
        }
    }

    pub fn signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    /// The Rust name of the type, which is also its Locus name.
    pub fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
        }
    }

    /// `2^bits`, the number of values of the type and the period of `wrap`.
    pub fn modulus(self) -> Integer {
        Integer::from(1u128 << self.bits())
    }

    /// The least value: `0`, or `-2^(bits - 1)` when signed.
    pub fn min(self) -> Integer {
        if self.signed() {
            Integer::from(1u128 << (self.bits() - 1)).neg()
        } else {
            Integer::zero()
        }
    }

    /// The greatest value: `2^bits - 1`, or `2^(bits - 1) - 1` when signed.
    pub fn max(self) -> Integer {
        let one = Integer::from(1i64);
        if self.signed() {
            Integer::from(1u128 << (self.bits() - 1)).sub(&one)
        } else {
            self.modulus().sub(&one)
        }
    }

    /// Whether `value` is a value of the type: `min <= value <= max`.
    pub fn contains(self, value: &Integer) -> bool {
        self.min() <= *value && *value <= self.max()
    }

    /// The one value of the type congruent to `value` modulo `2^bits`. This
    /// is what Rust's `as` computes from a wider type, and what the wrapping
    /// operations reduce their exact result by. It is an explicit reduction
    /// over `Integer`, so no width needs a special case: the remainder
    /// modulo `2^bits`, made non-negative, and then moved down by `2^bits`
    /// when it lies above `max`, which only a signed type has room for.
    pub fn wrap(self, value: &Integer) -> Integer {
        let modulus = self.modulus();
        // The remainder has the sign of `value` and a magnitude below the
        // modulus, so one addition brings a negative one into `0..modulus`.
        let mut reduced = value.rem(&modulus);
        if reduced.is_negative() {
            reduced = reduced.add(&modulus);
        }
        if reduced > self.max() {
            reduced = reduced.sub(&modulus);
        }
        debug_assert!(self.contains(&reduced));
        reduced
    }
}

#[cfg(test)]
mod tests {
    use super::{Integer, MachineInt};

    #[test]
    fn the_table_matches_rust() {
        let int = |value: i128| Integer::from(value);
        for (ty, min, max) in [
            (MachineInt::U8, 0, i128::from(u8::MAX)),
            (MachineInt::U16, 0, i128::from(u16::MAX)),
            (MachineInt::U32, 0, i128::from(u32::MAX)),
            (MachineInt::U64, 0, i128::from(u64::MAX)),
            (MachineInt::I8, i128::from(i8::MIN), i128::from(i8::MAX)),
            (MachineInt::I16, i128::from(i16::MIN), i128::from(i16::MAX)),
            (MachineInt::I32, i128::from(i32::MIN), i128::from(i32::MAX)),
            (MachineInt::I64, i128::from(i64::MIN), i128::from(i64::MAX)),
        ] {
            assert_eq!(ty.min(), int(min), "{}", ty.name());
            assert_eq!(ty.max(), int(max), "{}", ty.name());
            assert_eq!(ty.modulus(), int(1i128 << ty.bits()));
            assert!(ty.contains(&int(min)) && ty.contains(&int(max)));
            assert!(!ty.contains(&int(min - 1)) && !ty.contains(&int(max + 1)));
            assert_eq!(ty.wrap(&int(max + 1)), int(min));
            assert_eq!(ty.wrap(&int(min - 1)), int(max));
            assert_eq!(ty.wrap(&int(min)), int(min));
            assert_eq!(ty.wrap(&int(max)), int(max));
            assert_eq!(ty.wrap(&int(-1)), int(if ty.signed() { -1 } else { max }));
        }
    }
}
