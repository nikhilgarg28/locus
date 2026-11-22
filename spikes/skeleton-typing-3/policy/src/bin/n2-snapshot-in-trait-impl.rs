//! expect: runtime=ok check=ok
//! prints: run() => "locus::BoundedU32"
//! A snapshot of a type parameter taken in the body of a method of a
//! trait's `impl`. The trait's declaration says nothing of snapshots, so
//! the method may not ask for the bound, and lowering cannot add it.
use ::locus::locus;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    trait Show { fn show<T>(&self, x: T) -> &'static str; }
    struct S;
    impl Show for S {
        fn show<T>(&self, x: T) -> &'static str { type_of(&logic! { snap(x) }) }
    }
    fn run() -> &'static str { S.show(1u32) }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
