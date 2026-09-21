//! A seeded pseudorandom generator for the randomized tests. The build is
//! offline with one dependency, so this stands in for a crate: xorshift64*,
//! seeded through splitmix64. It uses only wrapping integer arithmetic, so a
//! seed gives the same sequence on every platform.
//!
//! This file is not part of `common/mod.rs`. A test includes it directly with
//! `#[path = "common/rng.rs"] mod rng;`.
// Each test that includes this file uses a different part of it.
#![allow(dead_code)]

use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Any seed is acceptable, zero included.
    pub fn new(seed: u64) -> Self {
        // Zero is the one state xorshift never leaves, and splitmix64 is a
        // bijection, so exactly one seed would land there.
        let state = splitmix64(seed);
        Self {
            state: if state == 0 { GOLDEN } else { state },
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state >> 12;
        self.state ^= self.state << 25;
        self.state ^= self.state >> 27;
        self.state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `0..bound`. The bias is below `bound / 2^64`.
    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0, "an empty range has no values");
        ((u128::from(self.next_u64()) * u128::from(bound)) >> 64) as u64
    }

    /// A value in `range`, which must not be empty.
    pub fn range(&mut self, range: Range<usize>) -> usize {
        assert!(range.start < range.end, "an empty range has no values");
        range.start + self.below((range.end - range.start) as u64) as usize
    }

    /// An element of `items`, which must not be empty.
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.range(0..items.len())]
    }

    /// True with probability `numerator / denominator`.
    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        self.below(denominator) < numerator
    }

    /// A generator for one part of a larger case, independent of how much of
    /// this one is used afterwards.
    pub fn fork(&mut self) -> Self {
        Self::new(self.next_u64())
    }
}

/// The seed of case `index` in a run that starts from `seed`. A failing case
/// reports this value, and it alone replays the case.
pub fn case_seed(seed: u64, index: u64) -> u64 {
    splitmix64(seed ^ splitmix64(index))
}

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn splitmix64(value: u64) -> u64 {
    let mut z = value.wrapping_add(GOLDEN);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
