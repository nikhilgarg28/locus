// expect: check=E0308 run=E0308
fn halve(n: u32, even: ::__locus::Proof![n % 2 == 0]) -> u32 {
    n / 2
}

fn as_pointer() -> fn(u32) -> u32 {
    halve //~ error: must be rejected
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
