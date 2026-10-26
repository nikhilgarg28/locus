// expect: check=ok run=ok
struct Interval {
    lo: u32,
    hi: u32,
    ordered: ::__locus::Proof![lo <= hi],
}

struct Checker;

impl Checker {
    fn order(&self, lo: u32, hi: u32) -> ::__locus::Proof![lo <= hi] {
        if lo > hi {
            panic!("unordered");
        }
        ::__locus::proof!([] [])
    }

    fn interval(&self, lo: u32, hi: u32) -> Interval {
        Interval { lo, hi, ordered: self.order(lo, hi) }
    }
}

fn width(lo: u32, hi: u32) -> u32 {
    let iv = Checker.interval(lo, hi);
    iv.hi - iv.lo
}

fn main() {
    println!("width(2, 5) = {}", width(2, 5));
    let late = std::panic::catch_unwind(|| width(5, 2));
    println!("width(5, 2) panicked = {}", late.is_err());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
