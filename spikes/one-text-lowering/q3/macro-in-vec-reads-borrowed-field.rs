// Question 5: inside vec![..], prove! reads the field that is mutably borrowed.
struct Pair {
    a: u32,
    b: u32,
}

fn bump_a(p: &mut Pair) -> u32 {
    let a = &mut p.a;
    let still: Vec<::__locus::Proof![p.a > 0]> = vec![prove!(p.a > 0)]; //~ error: p.a is mutably borrowed
    *a += 1;
    p.b
}
fn main() { let mut p = Pair { a: 1, b: 2 }; println!("bump_a = {}", bump_a(&mut p)); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=E0502 run=ok
