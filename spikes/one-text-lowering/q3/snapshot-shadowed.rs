// Question 3: a runtime binding shadows a ghost one inside a block.
struct Pair {
    a: u32,
    b: u32,
}

fn shadowed(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair); #[allow(unused_macros)] macro_rules! __locus_ghosts { (before; & $r:ident $($t:tt)*) => { ::__locus::observe(&$r) }; ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
    let n = {
        let mut before = Pair { a: 1, b: 2 }; #[allow(unused_macros)] macro_rules! __locus_ghosts { ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
        let bb = &mut before.b;
        let inner: Vec<::__locus::Proof![before.a == 1]> = vec![prove!(before.a == 1)]; ::__locus::observe!(&before.a);
        *bb += 1;
        inner.len()
    };
    let outer: Vec<::__locus::Proof![before.a == before.a]> = vec![prove!(before.a == before.a)]; ::__locus::observe!(&before);
    n + outer.len()
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("shadowed = {}", shadowed(&mut pair));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
// expect[precise]: check=E0609 run=ok
// expect[whole]: check=E0502 run=ok
