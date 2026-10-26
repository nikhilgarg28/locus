// Question 3: the program's own macro names a ghost binding in its body and is
// invoked where a runtime binding of the same name shadows it. Hygiene
// resolves the body's `before` where the macro is defined: the ghost.
struct Pair {
    a: u32,
    b: u32,
}

fn macro_body(pair: &mut Pair) -> usize {
    let before = ::__locus::data!([&pair] [] pair); #[allow(unused_macros)] macro_rules! __locus_ghosts { (before; & $r:ident $($t:tt)*) => { ::__locus::observe(&$r) }; ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
    macro_rules! ghost_a {
        () => {
            vec![prove!(before.a == before.a)]
        };
    }
    let n = {
        let mut before = Pair { a: 1, b: 2 }; #[allow(unused_macros)] macro_rules! __locus_ghosts { ($r:ident; $($b:tt)*) => { ::__locus::observe($($b)*) }; }
        let ba = &mut before.a;
        let v: Vec<::__locus::Proof![true]> = ghost_a!();
        *ba += 1;
        v.len()
    };
    n
}

fn main() {
    let mut pair = Pair { a: 1, b: 2 };
    println!("macro_body = {}", macro_body(&mut pair));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
// expect[precise]: check=E0609 run=ok
// (d) resolves the dispatch macro where ghost_a! is invoked, under the runtime
// shadow, while hygiene resolves the local `before` where ghost_a! is defined.
// expect[dispatch]: check=E0609 run=ok
