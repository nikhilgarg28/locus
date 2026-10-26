// expect: check=E0282 run=ok
// learns only later. Runtime code reading `p.a` there would be E0282 too.
#[derive(Default)]
struct Pair {
    a: u32,
    b: u32,
}

fn late_type() -> u32 {
    let p = Default::default();
    let h = ::__locus::proof!([&p.a] [] p.a == 0); //~ error: rustc's E0282, in the checking program only
    let q: Pair = p;
    q.a + q.b
}

fn main() {
    println!("late_type() = {}", late_type());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
