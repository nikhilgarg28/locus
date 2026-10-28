// expect: check=ok run=ok
struct Positive {
    value: u32,
    #[allow(dead_code)] nonzero: ::__locus::Proof![value > 0],
}

fn halve(n: u32, #[allow(unused_variables)] even: ::__locus::Proof![n % 2 == 0]) -> u32 {
    n / 2
}

fn uses() -> u32 {
    let n = 4;
    let even = ::__locus::proof!([&n] [] n % 2 == 0);
    let k = 6;
    #[allow(unused_variables)] let unused = ::__locus::proof!([&k] [] k > 0);
    let m = 7;
    let in_vec: Vec<::__locus::Proof![m > 0]> = vec![prove!(m > 0)]; ::__locus::observe!(&m);
    let p = Positive { value: 1, nonzero: ::__locus::proof!([] []) };
    halve(n, even) + p.value + in_vec.len() as u32
}

fn main() {
    println!("uses() = {}", uses());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// Lowering marked the ghost parameter, the unused ghost let, and the proof field.
