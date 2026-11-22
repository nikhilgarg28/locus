//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Prop", "locus::Prop", "locus::Prop", "locus::Bool")
//! `==` between two `Bool`s, and between a `Bool` and `true`, is a
//! `Prop` under the default, as every comparison is; `b === true` is one
//! too. A `Bool` alone stays a `Bool`.
use ::locus::locus;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str) {
        let a = true;
        let b = false;
        let two = logic! { a == b };
        let with_true = logic! { b == true };
        let same = logic! { b === true };
        let both: @(b == true && b === true && a != b) = _;
        let alone = logic! { a && !b };
        (type_of(&two), type_of(&with_true), type_of(&same), type_of(&alone))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
