//~ question: Inside another macro's input, may `prove!` read one field while another field is mutably borrowed?
//~ spec: observe.place.readable, compat.names.macros-expand
//~ rfc: 0001

struct Pair {
    a: u32,
    b: u32,
}

fn bump_a(p: &mut Pair, positive: ::__locus::Proof![p.b > 0]) -> u32 {
    let a = &mut p.a;
    let still: Vec<::__locus::Proof![p.b > 0]> = vec![prove!(p.b > 0)]; ::__locus::observe!(&p.b);
    *a += 1;
    p.b
}
fn main() { let mut p = Pair { a: 1, b: 2 }; let r = bump_a(&mut p, ::__locus::proof!([] [])); println!("bump_a = {r}, a = {}", p.a); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=ok run=ok
// expect[whole]: check=E0502 run=ok
