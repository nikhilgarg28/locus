// expect: check=E0382 run=ok
fn late() -> usize {
    let v = vec![1, 2, 3];
    let moved = v;
    let h = ::__locus::proof!([&v] [] v.len() == 3); //~ error: v has moved
    moved.len()
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
