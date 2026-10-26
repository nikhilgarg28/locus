// expect: check=E0369,E0277 run=E0369,E0277
#[derive(PartialEq, Hash)] //~ error: must be rejected
struct Counted {
    items: u32,
    count: Nat,
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
