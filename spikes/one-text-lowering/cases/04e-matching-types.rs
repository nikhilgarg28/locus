// expect: check=ok run=ok
// Hard case 4e: rustc accepts both; Locus must reject both from the facts
// (RFC 0002's boundary rule 2).
fn halve(n: u32, even: ::__locus::Proof![n % 2 == 0]) -> u32 {
    n / 2
}

fn pointer() -> u32 {
    let f: fn(u32, ::__locus::Proof![10 % 2 == 0]) -> u32 = halve; //~ error: Locus must reject; rustc cannot
    f(10, ::__locus::proof!([] []))
}

fn folded(proofs: Vec<::__locus::Proof![10 % 2 == 0]>) -> u32 {
    proofs.into_iter().fold(10, halve) //~ error: Locus must reject; rustc cannot
}

fn main() {
    let p: ::__locus::Proof![10 % 2 == 0] = ::__locus::proof!([] []);
    println!("pointer() = {}, folded = {}", pointer(), folded(vec![p, p]));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
