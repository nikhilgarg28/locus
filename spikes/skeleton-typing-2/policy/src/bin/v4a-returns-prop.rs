//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Prop", "locus::Bool", "locus::Bool")
//! A logic function whose body is a comparison. Declared to return `Prop`
//! it compiles as written. Declared to return `Bool` it needs the
//! proposition made a `Bool`: `decide(..)`, or `as Bool`.
use ::locus::locus;
locus! {
    logic fn small(n: Nat) -> Prop { n < 10 }
    logic fn small_decided(n: Nat) -> Bool { decide(n < 10) }
    logic fn small_cast(n: Nat) -> Bool { (n < 10) as Bool }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let n = 3u32;
        (type_of(&logic! { small(n) }), type_of(&logic! { small_decided(n) }), type_of(&logic! { small_cast(n) }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
