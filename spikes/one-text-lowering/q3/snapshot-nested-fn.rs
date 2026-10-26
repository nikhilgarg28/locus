// Question 3: a nested function binds a runtime parameter with a ghost's name.
// A local macro_rules is visible in nested items; a local binding is not.
struct Pair {
    a: u32,
    b: u32,
}

fn outer(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair); #[allow(unused_macros)] macro_rules! __locus_ghosts { (before; & $r:ident $($t:tt)*) => { ::__locus::observe(&$r) }; ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
    fn inner(mut before: Pair) -> usize { #[allow(unused_macros)] macro_rules! __locus_ghosts { ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
        let bb = &mut before.b;
        let v: Vec<::__locus::Proof![before.a == before.a]> = vec![prove!(before.a == before.a)]; ::__locus::observe!(&before.a);
        *bb += 1;
        v.len()
    }
    inner(Pair { a: 1, b: 2 })
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("outer = {}", outer(&mut pair));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// Lowering resets the dispatch macro at the top of the nested body.
// expect: check=ok run=ok
// expect[precise]: check=ok run=ok
// expect[whole]: check=E0502 run=ok
