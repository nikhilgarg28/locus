//! expect: runtime=ok check=ok
//! prints: run() => "locus::Int"
//! An operator whose left operand has a type that only the operator could
//! give: `fold` of an empty sequence, added to an `Int`. `Int` is the one
//! `Monoid` that can be added to an `Int`. rustc does not choose an
//! implementation for a type it does not know yet.
use ::locus::locus;

locus! {
    trait Monoid: Logical {
        logic fn unit() -> Self;
    }
    impl Monoid for Int {
        logic fn unit() -> Int { 0 }
    }
    logic fn fold<M: Monoid>(xs: Seq<M>) -> M { M::unit() }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(i: Int) -> &'static str {
        let total = logic! { fold(Seq::empty()) + i };
        type_of(&total)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
