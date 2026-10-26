// expect: check=ok run=ok
fn halve(n: u32, even: ::__locus::Proof![n % 2 == 0]) -> u32 {
    n / 2
}

fn lend() -> u32 {
    let f = std::hint::black_box(halve); //~ error: Locus must reject; rustc cannot
    f(10, ::__locus::proof!([] []))
}

fn main() {
    println!("lend() = {}", lend());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
