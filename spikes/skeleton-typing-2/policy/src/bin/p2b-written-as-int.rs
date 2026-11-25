//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Int", "locus::Int", "locus::Int", "locus::Int")
//! P2: the same with the conversion written. `n as Int < i` is a
//! comparison: the type of a cast has no generic arguments here.
//! `Nat - Nat` is an `Int` with nothing written: a result type.
use ::locus::locus;
locus! {
    logic fn negate(i: Int) -> Int { -i }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run(i: i32, n: u32) -> (&'static str, &'static str, &'static str, &'static str) {
        let sum = logic! { i + n as Int };
        let less = prove!(n as Int < i);
        let negated = logic! { negate(n as Int) };
        let from = logic! { Int::from(n as Nat) + i };
        let difference = logic! { n - n };
        (type_of(&sum), type_of(&negated), type_of(&from), type_of(&difference))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1, 2));
}
