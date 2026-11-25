//! expect: runtime=E0277 check=E0277
//! The same in a method of a trait's implementation. The trait is the
//! program's own, so the fix is to declare the bound in the trait (d2a).
use ::locus::locus;
locus! {
    trait Keeper { fn keep<T>(&self, x: &T) -> u32; }
    struct K;
    impl Keeper for K {
        fn keep<T>(&self, x: &T) -> u32 { let s = logic! { snap(x) }; 1 }
    }
    fn run() -> u32 { K.keep(&1u32) }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
