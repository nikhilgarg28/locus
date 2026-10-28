// Question 3: a proof hole inside another macro's input is only a token.
fn holes() -> usize {
    let proofs: Vec<::__locus::Proof![1 > 0]> = vec![_]; //~ error: rustc rejects `_` as an expression
    proofs.len()
}

fn main() {
    println!("holes() = {}", holes());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=error run=error
