// expect: check=ok run=ok
fn grow() -> usize {
    let mut values: Vec<Nat> = Vec::new();
    values.push(::__locus::data!([&values] [] values.len()));
    values.len()
}

fn main() {
    println!("grow() = {}", grow());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
