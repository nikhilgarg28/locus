//~ question: May a Locus macro be invoked inside another macro's input?
//~ spec: compat.names.macros-expand
//~ rfc: 0001

fn check(x: u32) -> usize {
    let proofs: Vec<::__locus::Proof![x == x]> = vec![prove!(x == x)]; ::__locus::observe!(&x);
    proofs.len()
}

//~ run: check(3) => 1
fn main() { println!("check(3) = {}", check(3)); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
