// expect: check=ok run=ok
// The old design: ghost parameters and their arguments removed. Lowering
// removed `h` from `x.go(1, h)` by guessing the callee from its name.
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
    fn go(&self, a: u32) -> u32 {
        a
    }
}

fn call(x: Thing) -> u32 {
    x.go(1)
}

fn main() {
    println!("call(Thing, _) = {}", call(Thing));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
