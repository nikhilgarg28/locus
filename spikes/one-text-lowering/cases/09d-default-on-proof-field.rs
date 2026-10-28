// expect: check=E0277 run=E0277
#[derive(Default)] //~ error: must be rejected
struct Positive {
    value: u32,
    nonzero: ::__locus::Proof![value > 0],
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
