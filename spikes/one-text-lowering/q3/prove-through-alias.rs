//~ question: May Locus's `prove!` be invoked under another name?
//~ spec: compat.names.macros-expand
//~ rfc: 0001

use prove as check;

fn same(x: u32) -> u32 {
    let proof: ::__locus::Proof![x == x] = check!(x == x); ::__locus::observe!(&x);
    x
}

//~ run: same(4) => 4
fn main() { println!("same(4) = {}", same(4)); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
