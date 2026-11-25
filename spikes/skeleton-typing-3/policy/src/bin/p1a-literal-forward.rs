//! expect: runtime=reject check=reject
//! P1: a ghost local initialized by a lone literal is an `Int`, read
//! forward. Passing it later where a `Nat` is wanted is an error in both
//! expansions: it does not take its type from the later use.
use ::locus::locus;
locus! {
    fn takes_nat(n: Nat) -> u32 { 1 }
    fn run() -> u32 {
        let zero = logic! { 0 };
        takes_nat(zero)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
