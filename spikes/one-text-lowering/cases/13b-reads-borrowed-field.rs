// expect: check=E0502 run=ok
struct Pair {
    a: u32,
    b: u32,
}

fn bump(p: &mut Pair) -> u32 {
    let a = &mut p.a;
    let h = ::__locus::proof!([&p.a] [] p.a > 0); //~ error: p.a is mutably borrowed
    *a += 1;
    p.b
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
