// expect: check=ok run=ok
fn identity<T>(x: T) -> T {
    x
}

fn relay(x: u32, positive: ::__locus::Proof![x > 0]) -> ::__locus::Proof![x > 0] {
    identity(positive)
}

fn main() {
    let p = relay(3, ::__locus::proof!([] []));
    let kept: Option<::__locus::Proof![3 > 0]> = Some(identity(p));
    println!("kept.is_some() = {}", kept.is_some());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
