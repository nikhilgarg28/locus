// expect: check=ok run=ok
struct Pair {
    a: u32,
    b: u32,
}

fn bump(p: &mut Pair) -> u32 {
    let a = &mut p.a;
    let h = ::__locus::proof!([&p.b] [] p.b > 0);
    *a += 1;
    p.b
}

fn main() {
    let mut p = Pair { a: 1, b: 2 };
    println!("bump = {}, a = {}", bump(&mut p), p.a);
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
