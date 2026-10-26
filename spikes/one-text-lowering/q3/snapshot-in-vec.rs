// Question 3's hard case: a logical snapshot read inside vec![..], beside a
// runtime root whose sibling field is mutably borrowed.
struct Pair {
    a: u32,
    b: u32,
}

fn snapshot(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair); #[allow(unused_macros)] macro_rules! __locus_ghosts { (before; & $r:ident $($t:tt)*) => { ::__locus::observe(&$r) }; ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
    let b = &mut pair.b;
    let proofs: Vec<::__locus::Proof![before.a == before.a]> = vec![prove!(before.a == before.a)]; ::__locus::observe!(&before);
    let more: Vec<::__locus::Proof![pair.a == before.a]> = vec![prove!(pair.a == before.a)]; ::__locus::observe!(&pair.a, &before);
    *b += 1;
    proofs.len() + more.len()
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("snapshot = {}, b = {}", snapshot(&mut pair), pair.b);
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
// expect[precise]: check=E0609 run=ok
// expect[whole]: check=E0502 run=ok
