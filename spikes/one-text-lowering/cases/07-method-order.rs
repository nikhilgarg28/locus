// expect: check=E0061 run=E0061
// (see 07-method-order.old.rs for the old design's text)
trait Go {
    fn go(self, a: u32) -> u32;
}

struct Thing;

impl Go for Thing {
    fn go(self, a: u32) -> u32 {
        a + 100
    }
}

impl Thing {
    fn go(&self, a: u32, h: ::__locus::Proof![a > 0]) -> u32 {
        a
    }
}

fn call(x: Thing, h: ::__locus::Proof![1 > 0]) -> u32 {
    x.go(1, h) //~ error: must fail loudly, not call Go::go
}

fn main() {
    println!("call(Thing, _) = {}", call(Thing, ::__locus::proof!([] [])));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
