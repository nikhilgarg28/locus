// Question 3's hard case: a logical snapshot read inside vec![..], beside a
// runtime root whose sibling field is mutably borrowed.
struct Pair {
    a: u32,
    b: u32,
}

fn snapshot(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair);
    let b = &mut pair.b;
    let proofs: Vec<::__locus::Proof![before.a == before.a]> = vec![::__locus::proof!([&before] [] before.a == before.a)]; ::__locus::observe!(&before);
    let more: Vec<::__locus::Proof![pair.a == before.a]> = vec![::__locus::proof!([&pair.a, &before] [] pair.a == before.a)]; ::__locus::observe!(&pair.a, &before);
    *b += 1;
    proofs.len() + more.len()
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("snapshot = {}, b = {}", snapshot(&mut pair), pair.b);
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// Option (c): lowering rewrote the invocations inside vec!'s input.
// expect: check=ok run=ok
