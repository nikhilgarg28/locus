// expect: check=ok run=ok
fn both(x: u32) -> u32 {
    let p = ::__locus::data!([&x] [] x > 0);
    let q = ::__locus::data!([&x] [] x < 10);
    let both = ::__locus::data!([&p, &q] [] p && q);
    x
}

fn main() {
    println!("both(3) = {}", both(3));
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
