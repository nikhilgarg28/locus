//! expect: runtime=reject check=reject
//! A type error in the contract of a trait method that has no body is
//! rustc's: a sequence where a number is wanted, in a runtime method's
//! result and in a lemma's parameter.
use ::locus::locus;
locus! {
    trait Shape {
        logic fn sides(self: &Self) -> Seq<Nat>;
        fn area(&self) -> (a: u32, @(a == Self::sides(self)));
        logic fn lemma(s: &Self, h: @(Self::sides(s) + 1 > 0)) -> @(True);
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
