//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Seq<locus::Nat>")
//! Required case 2, a trait of the program's own: the trait declares the
//! bound, on the method's parameter or on its own, and an implementation
//! takes snapshots under it.
use ::locus::locus;
locus! {
    trait Keeper {
        fn keep<T: Model>(&self, x: &T) -> Snap<T>;
    }
    trait Holds: Model {
        fn held(&self) -> Snap<Self> where Self: Sized;
    }
    struct K;
    impl Keeper for K {
        fn keep<T: Model>(&self, x: &T) -> Snap<T> { logic! { snap(x) } }
    }
    impl Holds for Vec<u32> {
        fn held(&self) -> Snap<Self> { logic! { snap(self) } }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str) {
        (type_of(&K.keep(&1u32)), type_of(&vec![1u32].held()))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
