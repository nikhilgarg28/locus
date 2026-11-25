//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat", "locus::Seq<locus::Nat>")
//! `if` in logical code accepts a `Bool` or any proposition (RFC 0002),
//! with one encoding, and its two branches still have one type.
use ::locus::locus;
locus! {
    logic fn pick(flag: Bool, a: Nat, b: Nat) -> Nat { if flag { a } else { b } }
    logic fn smaller(a: Nat, b: Nat) -> Nat { if a < b { a } else { b } }
    logic fn either(p: Prop, xs: Seq<Nat>) -> Seq<Nat> { if p && xs.len() > 0 { xs.skip(1 as Nat) } else { xs } }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let on = true;
        let v = vec![1u32, 2];
        let a = logic! { pick(on, 1 as Nat, 2 as Nat) };
        let b = logic! { smaller(a, a + 1) };
        let c = logic! { either(a < b, v) };
        (type_of(&a), type_of(&b), type_of(&c))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
