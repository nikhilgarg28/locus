//! expect: runtime=reject check=reject
//! A logic function declared to return `Bool` with a comparison as its
//! body: the comparison is a `Prop`. (Under p3-v0 it compiles.)
use ::locus::locus;
locus! {
    logic fn small(n: Nat) -> Bool { n < 10 }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
