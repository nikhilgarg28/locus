// expect: check=E0369 run=E0369
fn same(a: &Vec<Nat>, b: &Vec<Nat>) -> bool {
    a == b //~ error: must be rejected
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
