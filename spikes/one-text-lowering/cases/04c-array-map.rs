// expect: check=E0593 run=E0593
fn halve(n: u32, even: ::__locus::Proof![n % 2 == 0]) -> u32 {
    n / 2
}

fn mapped() -> [u32; 1] {
    [7].map(halve) //~ error: must be rejected
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
