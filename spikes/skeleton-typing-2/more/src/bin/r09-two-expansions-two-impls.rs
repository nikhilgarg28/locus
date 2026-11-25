//! expect: runtime=ok check=ok
//! prints: run() => "the fallback, for Inner"
//! For the split variant: a program in which rustc can select a different
//! implementation of a method that runs in each expansion. `Runs` is
//! implemented for every type that meets the logic-only trait `Spec`, and
//! `Inner` does not. Where everything meets `Spec`, the receiver
//! `Box<Inner>` itself has `Runs::describe`, and rustc never looks through
//! the box for `Fallback::describe`.
use ::locus::locus;

locus! {
    trait Spec {
        logic fn spec(x: Self) -> Nat;
    }
    impl Spec for u32 {
        logic fn spec(x: u32) -> Nat { x }
    }

    trait Runs {
        fn describe(&self) -> &'static str;
    }
    impl<T: Spec> Runs for T {
        fn describe(&self) -> &'static str { "the blanket implementation, for a type that meets Spec" }
    }

    trait Fallback {
        fn describe(&self) -> &'static str;
    }
    struct Inner;
    impl Fallback for Inner {
        fn describe(&self) -> &'static str { "the fallback, for Inner" }
    }

    fn run() -> &'static str {
        let boxed = Box::new(Inner);
        boxed.describe()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
