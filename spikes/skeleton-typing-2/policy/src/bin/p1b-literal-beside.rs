//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Int", "locus::Nat", "locus::Nat", "locus::Nat", "locus::Int", "locus::Int")
//! P1: a literal beside another operand takes that operand's type, an
//! index is a `Nat`, a written type is the type, and otherwise it is an
//! `Int`: alone, as an argument of a generic function, in both branches.
use ::locus::locus;
locus! {
    logic fn same<T: Logical>(t: T) -> T { t }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let n = 5u32;
        let v = vec![1u32, 2];
        let alone = logic! { 7 };
        let beside = logic! { n + 1 };
        let indexed = logic! { v[0] + 1 };
        let written = logic! { 3 as Nat };
        let generic = logic! { same(4) };
        let branches = logic! { if n === 5 { 1 } else { 2 } };
        (type_of(&alone), type_of(&beside), type_of(&indexed), type_of(&written), type_of(&generic), type_of(&branches))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
