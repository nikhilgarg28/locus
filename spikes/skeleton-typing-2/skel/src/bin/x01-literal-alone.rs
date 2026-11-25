//! expect: runtime=ok check=ok
//! prints: run() => "locus::Int"
//! `logic! { 0 }` with no annotation, and no later use that gives a type.
//! By RFC 0002 an integer literal is an `Int` unless a `Nat` is expected,
//! and nothing is expected here. Rust has no default for a literal of a
//! type that is not its own.
use ::locus::locus;

locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run() -> &'static str {
        let zero = logic! { 0 };
        let more = logic! { zero + 1 };
        type_of(&more)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
