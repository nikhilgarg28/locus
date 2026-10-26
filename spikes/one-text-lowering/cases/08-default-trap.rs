// expect: check=E0277 run=E0277
fn make<T: Default>(_: T) -> T {
    T::default()
}

fn positive(x: u32) -> ::__locus::Proof![x > 0] {
    if x == 0 {
        panic!("zero");
    }
    ::__locus::proof!([] [])
}

fn forge(x: u32) {
    let p = make(positive(x)); //~ error: must be rejected
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
