//! Tests for the numbers the kernel's evaluator will trust (build task K1):
//! `Natural` and `Integer` on their own, before any rule depends on them.
//! Up to the machine's limits they are compared with Rust's own arithmetic,
//! on a boundary set crossed with itself and on random pairs from a fixed
//! seed. Past those limits there is nothing to compare with, so the
//! algebraic identities are checked instead, with a few known values as
//! anchors. Set LOCUS_EXTENDED to run a hundred times as many random cases.

use std::cmp::Ordering;

use locus::kernel::{Integer, Natural};

/// Random cases per operation.
fn random_cases() -> usize {
    100_000 * scale()
}

/// Random cases past the machine's limits; these operands are much larger.
fn wide_cases() -> usize {
    2_000 * scale()
}

fn scale() -> usize {
    if std::env::var_os("LOCUS_EXTENDED").is_some() {
        100
    } else {
        1
    }
}

// --- Random values -----------------------------------------------------------

/// xorshift64*, local to this file so the cases never change under it.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    fn coin(&mut self) -> bool {
        self.below(2) == 0
    }

    /// A number of at most `max_bits` bits. The bit length is drawn first,
    /// so small, medium and near-limit values are equally likely, and half
    /// the time the top bit is set so the length is exact.
    fn magnitude(&mut self, max_bits: u32) -> u128 {
        let bits = self.below(u64::from(max_bits) + 1) as u32;
        if bits == 0 {
            return 0;
        }
        let raw = (u128::from(self.next()) << 64) | u128::from(self.next());
        let value = raw >> (128 - bits);
        if self.coin() {
            value | (1 << (bits - 1))
        } else {
            value
        }
    }

    fn i128(&mut self) -> i128 {
        match self.below(64) {
            0 => i128::MIN,
            1 => i128::MAX,
            _ => {
                let magnitude = self.magnitude(127) as i128;
                if self.coin() { magnitude } else { -magnitude }
            }
        }
    }

    /// Mostly independent, sometimes related, so that equal operands, zero
    /// sums, quotients of one and small divisors all occur.
    fn i128_pair(&mut self) -> (i128, i128) {
        let a = self.i128();
        let b = match self.below(10) {
            0 => a,
            1 => a.wrapping_neg(),
            2 => a.wrapping_add(1),
            3 => a.wrapping_sub(1),
            _ => self.i128(),
        };
        (a, b)
    }

    fn u128_pair(&mut self) -> (u128, u128) {
        let a = self.magnitude(128);
        let b = match self.below(10) {
            0 => a,
            1 => a.wrapping_add(1),
            2 => a.wrapping_sub(1),
            _ => self.magnitude(128),
        };
        (a, b)
    }

    /// Several hundred bits, built by multiplying and adding.
    fn wide_natural(&mut self) -> Natural {
        let mut value = Natural::from_u128(self.magnitude(128));
        for _ in 0..1 + self.below(5) {
            value = value
                .mul(&Natural::from_u128(self.magnitude(128)))
                .add(&Natural::from_u128(self.magnitude(128)));
        }
        value
    }

    /// One time in four a narrow value, so that wide quotients occur too.
    fn wide_integer(&mut self) -> Integer {
        let magnitude = if self.below(4) == 0 {
            Natural::from_u128(self.magnitude(128))
        } else {
            self.wide_natural()
        };
        Integer::from_parts(self.coin(), magnitude)
    }
}

// --- Boundary values ---------------------------------------------------------

/// The limbs are 32 bits wide, so every multiple of 32 is a limb boundary;
/// 7, 8, 15 and 16 are there for the machine types.
const POWERS: [u32; 12] = [7, 8, 15, 16, 31, 32, 33, 63, 64, 65, 95, 96];

/// Two steps to either side, so that a MAX of `2^k - 1` has both neighbours.
fn around(power: u128) -> [u128; 5] {
    [power - 2, power - 1, power, power + 1, power + 2]
}

/// 0, 1, 2, the values around every power, and the top of `u128`. This holds
/// the MAX of every unsigned and signed machine type with its neighbours.
fn natural_boundaries() -> Vec<u128> {
    let mut values = vec![0, 1, 2, 3, 10, u128::MAX - 1, u128::MAX];
    for power in POWERS.into_iter().chain([127]) {
        values.extend(around(1 << power));
    }
    values.sort_unstable();
    values.dedup();
    values
}

/// The same powers on both sides of zero, which holds the MIN of every
/// signed machine type with its neighbours, and the two ends of `i128`.
fn integer_boundaries() -> Vec<i128> {
    let mut values = vec![0, 1, 2, 3, 10, i128::MIN, i128::MAX - 1, i128::MAX];
    for power in POWERS {
        values.extend(around(1 << power).map(|value| value as i128));
    }
    values.extend(around(1 << 126).map(|value| value as i128));
    for value in values.clone() {
        values.push(value.wrapping_neg());
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[test]
fn the_boundary_sets_hold_the_limits_of_every_machine_type() {
    let naturals = natural_boundaries();
    for max in [
        u128::from(u8::MAX),
        u128::from(u16::MAX),
        u128::from(u32::MAX),
        u128::from(u64::MAX),
    ] {
        for value in [max - 1, max, max + 1] {
            assert!(naturals.contains(&value), "{value}");
        }
    }
    assert!(naturals.contains(&u128::MAX));

    let integers = integer_boundaries();
    for (min, max) in [
        (i128::from(i8::MIN), i128::from(i8::MAX)),
        (i128::from(i16::MIN), i128::from(i16::MAX)),
        (i128::from(i32::MIN), i128::from(i32::MAX)),
        (i128::from(i64::MIN), i128::from(i64::MAX)),
    ] {
        for value in [min - 1, min, min + 1, max - 1, max, max + 1] {
            assert!(integers.contains(&value), "{value}");
        }
    }
    for value in [-2, -1, 0, 1, 2, i128::MIN, i128::MAX] {
        assert!(integers.contains(&value), "{value}");
    }
}

// --- Integer against i128 ----------------------------------------------------
//
// Where i128 overflows, the exact answer does not fit in an i128, so
// `to_i128` must say `None` just as `checked_*` does. No pair is skipped.

fn int(value: i128) -> Integer {
    let integer = Integer::from(value);
    // Rust's own printing is the independent witness that the conversion,
    // on which every comparison below rests, is right.
    assert_eq!(integer.to_string(), value.to_string());
    assert_eq!(integer.to_i128(), Some(value));
    assert_eq!(integer.is_negative(), value < 0);
    assert_eq!(integer.is_zero(), value == 0);
    integer
}

/// Zero is never negative, whatever produced it.
fn canonical(value: Integer) -> Integer {
    if value.is_zero() {
        assert!(!value.is_negative());
        assert_eq!(value, Integer::zero());
    }
    value
}

fn check_add(a: i128, b: i128) {
    let sum = canonical(int(a).add(&int(b)));
    assert_eq!(sum.to_i128(), a.checked_add(b), "{a} + {b}");
}

fn check_sub(a: i128, b: i128) {
    let difference = canonical(int(a).sub(&int(b)));
    assert_eq!(difference.to_i128(), a.checked_sub(b), "{a} - {b}");
}

fn check_mul(a: i128, b: i128) {
    let product = canonical(int(a).mul(&int(b)));
    assert_eq!(product.to_i128(), a.checked_mul(b), "{a} * {b}");
}

fn check_neg(a: i128) {
    let negation = canonical(int(a).neg());
    assert_eq!(negation.to_i128(), a.checked_neg(), "-{a}");
}

fn check_div(a: i128, b: i128) {
    let (x, y) = (int(a), int(b));
    let quotient = canonical(x.div(&y));
    // Only MIN / -1 overflows, and in the logic it is just a number.
    let expected = if b == 0 { Some(0) } else { a.checked_div(b) };
    assert_eq!(quotient.to_i128(), expected, "{a} / {b}");
    if expected.is_none() {
        assert_eq!(quotient, Integer::from(i128::MAX).add(&Integer::from(1i64)));
    }
    assert_eq!(quotient, x.div_rem(&y).0);
}

fn check_rem(a: i128, b: i128) {
    let (x, y) = (int(a), int(b));
    let remainder = canonical(x.rem(&y));
    // `wrapping_rem` differs from `%` only at MIN % -1, where `%` refuses
    // although the remainder, zero, fits.
    let expected = if b == 0 { a } else { a.wrapping_rem(b) };
    assert_eq!(remainder.to_i128(), Some(expected), "{a} % {b}");
    assert_eq!(remainder, x.div_rem(&y).1);
}

fn check_cmp(a: i128, b: i128) {
    let (x, y) = (int(a), int(b));
    assert_eq!(x.cmp(&y), a.cmp(&b), "{a} cmp {b}");
    assert_eq!(x == y, a == b, "{a} == {b}");
}

#[test]
fn integers_agree_with_i128_on_the_boundary_set() {
    let values = integer_boundaries();
    for &a in &values {
        check_neg(a);
        assert_eq!(a.to_string().parse(), Ok(int(a)));
        for &b in &values {
            check_add(a, b);
            check_sub(a, b);
            check_mul(a, b);
            check_div(a, b);
            check_rem(a, b);
            check_cmp(a, b);
        }
    }
}

fn on_random_pairs(seed: u64, check: fn(i128, i128)) {
    let mut rng = Rng(seed);
    for _ in 0..random_cases() {
        let (a, b) = rng.i128_pair();
        check(a, b);
    }
}

#[test]
fn integer_addition_agrees_with_i128_on_random_pairs() {
    on_random_pairs(0x9E37_79B9_7F4A_7C15, check_add);
}

#[test]
fn integer_subtraction_agrees_with_i128_on_random_pairs() {
    on_random_pairs(0xD1B5_4A32_D192_ED03, check_sub);
}

#[test]
fn integer_multiplication_agrees_with_i128_on_random_pairs() {
    on_random_pairs(0x8CB9_2BA7_2F3D_8DD7, check_mul);
    // Independent pairs mostly overflow. Here the bit lengths sum to at most
    // 126, so every product fits and is compared.
    let mut rng = Rng(0xABC9_8388_FB8F_AC03);
    for _ in 0..random_cases() {
        let left_bits = rng.below(127) as u32;
        let a = rng.magnitude(left_bits) as i128;
        let b = rng.magnitude(126 - left_bits) as i128;
        let (a, b) = (
            if rng.coin() { a } else { -a },
            if rng.coin() { b } else { -b },
        );
        assert!(a.checked_mul(b).is_some());
        check_mul(a, b);
    }
}

#[test]
fn integer_division_agrees_with_i128_on_random_pairs() {
    on_random_pairs(0x2545_F491_4F6C_DD1D, check_div);
}

#[test]
fn integer_remainder_agrees_with_i128_on_random_pairs() {
    on_random_pairs(0x6A09_E667_F3BC_C909, check_rem);
}

#[test]
fn integer_comparison_and_negation_agree_with_i128_on_random_pairs() {
    on_random_pairs(0xBB67_AE85_84CA_A73B, |a, b| {
        check_cmp(a, b);
        check_neg(a);
    });
}

#[test]
fn integer_conversions_agree_with_the_machine_types() {
    let mut rng = Rng(0x3C6E_F372_FE94_F82B);
    let mut values = integer_boundaries();
    values.extend((0..10_000).map(|_| rng.i128()));
    for value in values {
        let integer = int(value);
        assert_eq!(value.to_string().parse(), Ok(integer.clone()));
        assert_eq!(integer.magnitude().to_u128(), Some(value.unsigned_abs()));
        assert_eq!(
            integer.to_natural().and_then(|natural| natural.to_u128()),
            u128::try_from(value).ok()
        );
        if let Ok(narrow) = i64::try_from(value) {
            assert_eq!(Integer::from(narrow), integer);
        }
        if let Ok(unsigned) = u128::try_from(value) {
            assert_eq!(Integer::from(unsigned), integer);
            assert_eq!(Integer::from(Natural::from_u128(unsigned)), integer);
        }
    }
    let top = Integer::from(u128::MAX);
    assert_eq!(top.to_string(), u128::MAX.to_string());
    assert_eq!(top.to_i128(), None);
    assert_eq!(top.neg().to_i128(), None);
    assert_eq!(top.neg().to_string(), format!("-{}", u128::MAX));
}

// --- Natural against u128 ----------------------------------------------------

fn nat(value: u128) -> Natural {
    let natural = Natural::from_u128(value);
    assert_eq!(natural.to_string(), value.to_string());
    assert_eq!(natural.to_u128(), Some(value));
    natural
}

fn check_natural(a: u128, b: u128) {
    let (x, y) = (nat(a), nat(b));
    assert_eq!(x.add(&y).to_u128(), a.checked_add(b), "{a} + {b}");
    assert_eq!(x.mul(&y).to_u128(), a.checked_mul(b), "{a} * {b}");
    assert_eq!(x.cmp(&y), a.cmp(&b), "{a} cmp {b}");
    assert_eq!(x == y, a == b, "{a} == {b}");

    // A difference that exists is no larger than `a`, so it always fits.
    let difference = x.checked_sub(&y);
    assert_eq!(difference.is_some(), a >= b, "{a} - {b}");
    assert_eq!(
        difference.and_then(|natural| natural.to_u128()),
        a.checked_sub(b),
        "{a} - {b}"
    );

    match x.div_rem(&y) {
        None => assert_eq!(b, 0, "{a} / {b}"),
        Some((quotient, remainder)) => {
            assert_eq!(quotient.to_u128(), a.checked_div(b), "{a} / {b}");
            assert_eq!(remainder.to_u128(), a.checked_rem(b), "{a} % {b}");
        }
    }
}

fn check_natural_conversions(value: u128) {
    let natural = nat(value);
    assert_eq!(value.to_string().parse(), Ok(natural.clone()));
    assert_eq!(natural.is_zero(), value == 0);
    assert_eq!(natural.bit_length(), (128 - value.leading_zeros()) as usize);
    assert_eq!(natural.low_byte(), value as u8);
    assert_eq!(natural.to_u64(), u64::try_from(value).ok());
    if let Ok(narrow) = u64::try_from(value) {
        assert_eq!(Natural::from(narrow), natural);
    }
    assert_eq!(natural.succ().to_u128(), value.checked_add(1));
}

#[test]
fn naturals_agree_with_u128_on_the_boundary_set() {
    let values = natural_boundaries();
    for &a in &values {
        check_natural_conversions(a);
        for &b in &values {
            check_natural(a, b);
        }
    }
}

#[test]
fn naturals_agree_with_u128_on_random_pairs() {
    let mut rng = Rng(0xA54F_F53A_5F1D_36F1);
    for _ in 0..random_cases() {
        let (a, b) = rng.u128_pair();
        check_natural(a, b);
        check_natural_conversions(a);
    }
}

#[test]
fn natural_products_that_fit_agree_with_u128() {
    let mut rng = Rng(0x510E_527F_ADE6_82D1);
    for _ in 0..random_cases() {
        let left_bits = rng.below(129) as u32;
        let (a, b) = (rng.magnitude(left_bits), rng.magnitude(128 - left_bits));
        assert_eq!(nat(a).mul(&nat(b)).to_u128(), Some(a * b), "{a} * {b}");
    }
}

// --- Past the machine's limits -----------------------------------------------

fn check_integer_identities(a: &Integer, b: &Integer, c: &Integer) {
    let zero = Integer::zero();
    let sum = canonical(a.add(b));
    let difference = canonical(a.sub(b));
    let product = canonical(a.mul(b));

    assert_eq!(&canonical(sum.sub(b)), a, "(a + b) - b");
    assert_eq!(&canonical(difference.add(b)), a, "(a - b) + b");
    assert_eq!(canonical(b.sub(a)), difference.neg(), "b - a == -(a - b)");
    assert_eq!(canonical(a.add(&a.neg())), zero, "a + -a");
    assert_eq!(&a.neg().neg(), a, "--a");

    assert_eq!(sum, b.add(a), "a + b == b + a");
    assert_eq!(product, b.mul(a), "a * b == b * a");
    assert_eq!(sum.add(c), a.add(&b.add(c)), "(a + b) + c == a + (b + c)");
    assert_eq!(
        product.mul(c),
        a.mul(&b.mul(c)),
        "(a * b) * c == a * (b * c)"
    );
    assert_eq!(
        a.mul(&b.add(c)),
        product.add(&a.mul(c)),
        "a * (b + c) == a * b + a * c"
    );

    // Division undoes multiplication, and the division identity holds for
    // every divisor, zero included.
    let (quotient, remainder) = a.div_rem(b);
    let (quotient, remainder) = (canonical(quotient), canonical(remainder));
    assert_eq!(quotient, a.div(b));
    assert_eq!(remainder, a.rem(b));
    assert_eq!(
        &quotient.mul(b).add(&remainder),
        a,
        "a == (a / b) * b + a % b"
    );
    if b.is_zero() {
        assert_eq!(quotient, zero, "a / 0");
        assert_eq!(&remainder, a, "a % 0");
    } else {
        assert_eq!(&canonical(product.div(b)), a, "(a * b) / b");
        assert_eq!(canonical(product.rem(b)), zero, "(a * b) % b");
        assert!(remainder.magnitude() < b.magnitude(), "|a % b| < |b|");
        assert!(
            remainder.is_zero() || remainder.is_negative() == a.is_negative(),
            "a % b has the sign of a"
        );
        // Truncation toward zero: |a / b| * |b| <= |a|.
        assert!(&quotient.magnitude().mul(b.magnitude()) <= a.magnitude());
        assert!(
            quotient.is_zero() || quotient.is_negative() == (a.is_negative() != b.is_negative()),
            "a / b has the product of the signs"
        );
    }

    let expected = if difference.is_zero() {
        Ordering::Equal
    } else if difference.is_negative() {
        Ordering::Less
    } else {
        Ordering::Greater
    };
    assert_eq!(a.cmp(b), expected, "cmp is the sign of a - b");
    assert_eq!(b.cmp(a), expected.reverse());
    assert_eq!(a == b, expected == Ordering::Equal);

    assert_eq!(a.to_string().parse().as_ref(), Ok(a), "print then parse");
}

#[test]
fn integer_identities_hold_past_i128() {
    let mut rng = Rng(0x1F83_D9AB_FB41_BD6B);
    for _ in 0..wide_cases() {
        let a = rng.wide_integer();
        let b = match rng.below(16) {
            0 => Integer::zero(),
            1 => a.clone(),
            2 => a.neg(),
            3 => a.add(&Integer::from(1i64)),
            _ => rng.wide_integer(),
        };
        let c = rng.wide_integer();
        check_integer_identities(&a, &b, &c);
    }
}

#[test]
fn integer_identities_hold_around_the_limits_of_the_machine_types() {
    // The i128 boundary set, and the same shapes one and two limbs further
    // out than any machine type reaches.
    let mut values: Vec<Integer> = integer_boundaries()
        .into_iter()
        .map(Integer::from)
        .collect();
    let one = Integer::from(1i64);
    let mut rng = Rng(0x5BE0_CD19_137E_2179);
    for wide in [
        Integer::from(u128::MAX),
        Integer::from(u128::MAX).add(&one),
        Integer::from(u128::MAX).add(&one).add(&one),
        Integer::from(u128::MAX).mul(&Integer::from(u128::MAX)),
        Integer::from(i128::MIN).mul(&Integer::from(i128::MIN)),
        Integer::from(i128::MIN).sub(&one),
        Integer::from(i128::MAX).add(&one),
    ] {
        values.push(wide.neg());
        values.push(wide);
    }
    for a in &values {
        for b in &values {
            check_integer_identities(a, b, &rng.wide_integer());
        }
    }
}

#[test]
fn natural_identities_hold_past_u128() {
    let mut rng = Rng(0xCBBB_9D5D_C105_9ED8);
    for _ in 0..wide_cases() {
        let a = rng.wide_natural();
        let b = match rng.below(8) {
            0 => Natural::zero(),
            1 => a.clone(),
            2 => a.succ(),
            3 => Natural::from_u128(rng.magnitude(128)),
            _ => rng.wide_natural(),
        };
        let c = rng.wide_natural();

        let sum = a.add(&b);
        let product = a.mul(&b);
        assert_eq!(sum.checked_sub(&b).as_ref(), Some(&a), "(a + b) - b");
        assert_eq!(sum, b.add(&a));
        assert_eq!(product, b.mul(&a));
        assert_eq!(a.mul(&b.add(&c)), product.add(&a.mul(&c)));

        // Subtraction is defined exactly when the order says so, and the
        // order is the one addition gives: a < a + c + 1.
        assert_eq!(a.checked_sub(&b).is_some(), a >= b);
        assert_eq!(b.checked_sub(&a).is_some(), b >= a);
        assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
        assert_eq!(a.cmp(&b) == Ordering::Equal, a == b);
        assert!(a < a.add(&c).succ());
        assert_eq!(a.checked_sub(&a.add(&c).succ()), None);

        match a.div_rem(&b) {
            None => assert!(b.is_zero()),
            Some((quotient, remainder)) => {
                assert!(remainder < b, "a % b < b");
                assert_eq!(
                    quotient.mul(&b).add(&remainder),
                    a,
                    "a == (a / b) * b + a % b"
                );
                assert_eq!(product.div_rem(&b), Some((a.clone(), Natural::zero())));
                // The remainder survives adding a multiple of the divisor.
                assert_eq!(
                    product.add(&remainder).div_rem(&b),
                    Some((a.clone(), remainder))
                );
            }
        }

        // 2^k needs k + 1 bits, and a product needs the sum of the lengths
        // or one bit less.
        if !a.is_zero() && !b.is_zero() {
            let bits = product.bit_length();
            assert!(
                bits == a.bit_length() + b.bit_length()
                    || bits + 1 == a.bit_length() + b.bit_length()
            );
        }
        assert_eq!(a.to_string().parse().as_ref(), Ok(&a), "print then parse");
    }
}

#[test]
fn known_values_past_the_machine_limits_are_printed_and_parsed_exactly() {
    // Factorials: multiplication by small numbers, undone by division.
    let mut factorial = Natural::from(1);
    for factor in 1..=30u64 {
        factorial = factorial.mul(&Natural::from(factor));
        if factor == 25 {
            assert_eq!(factorial.to_string(), "15511210043330985984000000");
        }
    }
    assert_eq!(factorial.to_string(), "265252859812191058636308480000000");
    for factor in 1..=30u64 {
        let (quotient, remainder) = factorial.div_rem(&Natural::from(factor)).unwrap();
        assert!(remainder.is_zero());
        factorial = quotient;
    }
    assert_eq!(factorial, Natural::from(1));

    // Powers of ten: the printed form is known without any arithmetic.
    let ten = Natural::from(10);
    let mut power = Natural::from(1);
    for exponent in 0..=60usize {
        let text = format!("1{}", "0".repeat(exponent));
        assert_eq!(power.to_string(), text);
        assert_eq!(text.parse(), Ok(power.clone()));
        if exponent > 0 {
            // 10^n - 1 is n nines: a borrow through every limb.
            let nines = power.checked_sub(&Natural::from(1)).unwrap();
            assert_eq!(nines.to_string(), "9".repeat(exponent));
            let (quotient, remainder) = nines.div_rem(&ten).unwrap();
            let expected = if exponent == 1 {
                "0".to_string()
            } else {
                "9".repeat(exponent - 1)
            };
            assert_eq!(quotient.to_string(), expected);
            assert_eq!(remainder, Natural::from(9));
        }
        power = power.mul(&ten);
    }

    // Powers of two: 2^k needs k + 1 bits, and doubling is adding.
    let mut power = Natural::from(1);
    for exponent in 0..=300usize {
        assert_eq!(power.bit_length(), exponent + 1);
        assert_eq!(power.mul(&Natural::from(2)), power.add(&power));
        power = power.add(&power);
    }
    assert_eq!(
        power.to_string(),
        "4074071952668972172536891376818756322102936787331872501272280898708762599526673412366794752"
    );

    let text = "-340282366920938463463374607431768211456";
    let integer: Integer = text.parse().unwrap();
    assert_eq!(
        integer,
        Integer::from(u128::MAX).add(&Integer::from(1i64)).neg()
    );
    assert_eq!(integer.to_string(), text);
    assert_eq!("-0".parse(), Ok(Integer::zero()));
    assert_eq!("-000123".parse(), Ok(Integer::from(-123i64)));
    for text in [
        "", "-", "--1", "+1", "1-", "- 1", "1_000", "0x10", "1e3", " 1", "1 ",
    ] {
        assert!(text.parse::<Integer>().is_err(), "{text:?}");
        assert!(text.parse::<Natural>().is_err(), "{text:?}");
    }
    assert!("-1".parse::<Natural>().is_err());
}
