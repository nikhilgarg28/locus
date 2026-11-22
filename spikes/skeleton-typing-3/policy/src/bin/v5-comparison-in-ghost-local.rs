//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Prop", "locus::Int")
//! A comparison stored in a ghost local with no annotation is a `Prop`,
//! and is then used in `@(..)` and as the condition of an `if`.
use ::locus::locus;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str) {
        let n = 3u32;
        let small = logic! { n < 10 };
        let known: @(small) = _;
        let chosen = logic! { if small { 1 } else { 2 } };
        (type_of(&small), type_of(&chosen))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
