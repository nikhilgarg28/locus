// expect: check=ok run=ok
fn halve(n: u32, even: ::__locus::Proof![n % 2 == 0]) -> u32 {
    n / 2
}

fn pick() -> u32 {
    let f = halve;
    f(10, ::__locus::proof!([] []))
}

fn main() {
    println!("pick() = {}", pick());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
