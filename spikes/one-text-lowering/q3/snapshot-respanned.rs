// Question 3: the hard case inside a function whose attribute macro rebuilds
// every token with its own call-site span, which moves every position.
use respan::respan;

struct Pair {
    a: u32,
    b: u32,
}

#[respan]
fn respanned(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair); #[allow(unused_macros)] macro_rules! __locus_ghosts { (before; & $r:ident $($t:tt)*) => { ::__locus::observe(&$r) }; ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
    let b = &mut pair.b;
    let proofs: Vec<::__locus::Proof![before.a == before.a]> = vec![prove!(before.a == before.a)]; ::__locus::observe!(&before);
    let more: Vec<::__locus::Proof![pair.a == before.a]> = vec![prove!(pair.a == before.a)]; ::__locus::observe!(&pair.a, &before);
    *b += 1;
    proofs.len() + more.len()
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("respanned = {}", respanned(&mut pair));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
// expect[precise]: check=E0609 run=ok
// expect[whole]: check=E0502 run=ok
// expect[table]: check=E0609 run=ok
