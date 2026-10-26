// The new design: one lowered text, compiled against both library flavors.
use token_sensitive::token_sensitive;

#[token_sensitive]
fn answer() -> u32 {
    let x = 0;
    let h = ::__locus::proof!([&x] [] x == x);
    x
}

fn main() {
    println!("answer() = {}", answer());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
