//! expect: runtime=ok check=ok
//! prints: run() => ("a04_generic_conditional::Pair_Snap<locus::Nat, locus::Seq<locus::Nat>>", "locus::Nat", "locus::Seq<locus::Int>", "a04_generic_conditional::Pair_Snap<a04_generic_conditional::Stack_Snap<locus::Seq<locus::Nat>>, locus::Seq<a04_generic_conditional::Stack_Snap<locus::Seq<locus::Nat>>>>")
//! A generic type's implementation is conditional on its parameter's:
//! `unsafe impl<T: Model> Model for Pair<T>`. Generic code reads it under
//! the bound it writes, and gives the type concrete code has.
use ::locus::locus;
locus! {
    struct Pair<T> { first: T, rest: Vec<T> }
    struct Stack { items: Vec<u32> }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
    }
    fn first_of<T: Model>(p: &Pair<T>) -> T::Logic { logic! { p.first } }
    fn rest_of<T: Model>(p: &Pair<T>) -> Seq<T::Logic> { logic! { p.rest } }
    fn whole<T: Model>(p: &Pair<T>) -> Snap<Pair<T>> { logic! { snap(p) } }
    logic fn takes_nat(n: Nat) -> Nat { n }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str) {
        let p = Pair { first: 1u32, rest: vec![2u32] };
        let q = Pair { first: -1i64, rest: vec![2i64] };
        let s = Pair { first: Stack { items: vec![] }, rest: vec![] };
        let concrete = logic! { p };
        let generic = first_of(&p);
        let used = logic! { takes_nat(generic) };
        (type_of(&concrete), type_of(&used), type_of(&rest_of(&q)), type_of(&whole(&s)))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
