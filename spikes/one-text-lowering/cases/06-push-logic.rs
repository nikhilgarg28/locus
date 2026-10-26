// expect: check=ok run=ok
fn annotated() -> usize {
    let mut values: Vec<Nat> = Vec::new();
    values.push(::__locus::data!([] [] 3));
    values.len()
}

fn unannotated() -> usize {
    let mut values = Vec::new(); // RFC 0002: logical types flow forward only
    values.push(::__locus::data!([] [] 3));
    values.len()
}

fn conflated() -> usize {
    let naturals: Vec<Nat> = Vec::new();
    let sequences: Vec<Seq<Int>> = naturals; //~ error: Locus must reject: Vec<Nat> is not Vec<Seq<Int>>
    sequences.len()
}

fn main() {
    println!("{} {} {}", annotated(), unannotated(), conflated());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
