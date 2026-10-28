// expect: check=ok run=ok
// The old design: a result that is entirely ghost becomes `()`.
fn make<T: Default>(_: T) -> T {
    T::default()
}

fn positive(x: u32) {
    if x == 0 {
        panic!("zero");
    }

}

fn forge(x: u32) {
    let p = make(positive(x)); // accepted: T = (), and () is Default
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
