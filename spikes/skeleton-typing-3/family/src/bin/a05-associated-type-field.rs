//! expect: runtime=ok check=ok
//! prints: run() => ("a05_associated_type_field::Kept_Snap<locus::Nat, locus::Nat>", "locus::Nat")
//! A field whose type is an associated type of a parameter: the generated
//! implementation is conditional on `F::Out: Model`, and a function that
//! reads the struct writes that bound.
use ::locus::locus;
locus! {
    trait Family { type Out; }
    struct Bytes;
    impl Family for Bytes { type Out = u8; }
    struct Kept<F: Family> { out: F::Out, n: u32 }
    fn kept<F: Family>(k: &Kept<F>) -> Snap<Kept<F>> where F::Out: Model { logic! { snap(k) } }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str) {
        let k: Kept<Bytes> = Kept { out: 1, n: 2 };
        let s = kept(&k);
        (type_of(&s), type_of(&logic! { s.out }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
