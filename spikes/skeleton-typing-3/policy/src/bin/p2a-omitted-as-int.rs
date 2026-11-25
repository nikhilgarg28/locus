//! expect: runtime=reject check=reject
//! P2: a `Nat` and an `Int` under one operator. The conversion is
//! written, `n as Int`; these omit it, in arithmetic, in a comparison and
//! at an argument.
use ::locus::locus;
locus! {
    logic fn negate(i: Int) -> Int { -i }
    fn run(i: i32, n: u32) -> u32 {
        let sum = logic! { i + n };
        let less = prove!(n < i);
        let negated = logic! { negate(n) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1, 2));
}
