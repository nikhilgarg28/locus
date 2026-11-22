//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Seq<locus::Nat>", "locus::Seq<locus::Nat>", "locus::Seq<locus::Nat>", "locus::Seq<locus::Nat>", "locus::Nat", "locus::Nat")
//! Required case 7: the model of a type is the same wherever it is read:
//! in the module that declares it, in a module that imports only the type,
//! through a bounded parameter, and through a generic function of another
//! module. And a `u32` read through a parameter is a `u32` read directly.
use ::locus::locus;
mod declares {
    use ::locus::locus;
    locus! {
        pub struct Stack { pub items: Vec<u32> }
        impl Model for Stack {
            type Logic = Seq<Nat>;
            logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
        }
        pub fn here(s: &Stack) -> &'static str { super::type_of(&logic! { *s }) }
    }
}
mod generic {
    use ::locus::locus;
    locus! {
        pub fn read<T: Model>(x: &T) -> T::Logic { logic! { *x } }
    }
}
fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
locus! {
    fn bounded<T: Model>(x: &T) -> &'static str { type_of(&logic! { *x }) }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let s = declares::Stack { items: vec![1] };
        let n = 1u32;
        (declares::here(&s), type_of(&logic! { s }), bounded(&s), type_of(&generic::read(&s)), bounded(&n), type_of(&logic! { n }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
