// Question 3: ghost and runtime roots passed through the program's own macro.
struct Pair {
    a: u32,
    b: u32,
}

macro_rules! same_a {
    ($e:expr) => {
        vec![prove!($e.a == $e.a)]
    };
}

fn through_own_macro(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair); #[allow(unused_macros)] macro_rules! __locus_ghosts { (before; & $r:ident $($t:tt)*) => { ::__locus::observe(&$r) }; ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
    let b = &mut pair.b;
    let ghost: Vec<::__locus::Proof![before.a == before.a]> = same_a!(before); ::__locus::observe!(&before);
    let runtime: Vec<::__locus::Proof![pair.a == pair.a]> = same_a!(pair); ::__locus::observe!(&pair.a);
    *b += 1;
    ghost.len() + runtime.len()
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("through_own_macro = {}", through_own_macro(&mut pair));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
// expect[precise]: check=E0609 run=ok
// expect[whole]: check=E0502 run=ok
