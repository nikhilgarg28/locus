//! expect: runtime=E0119 check=E0119
//! Required case 6, the other side: a second implementation for a type
//! that lowering wrote one for is a conflict, whoever writes it.
use ::locus::locus;
locus! {
    struct Point { x: u32 }
    fn run() -> u32 { 1 }
}
unsafe impl ::locus::Model for Point {
    type Snap = ::locus::Nat;
    type Logic = ::locus::Nat;
    fn __logic_model(s: ::locus::Nat) -> ::locus::Nat { s }
    fn __logic_valid(_: ::locus::Nat) -> ::locus::Prop { ::locus::True }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
