//! expect: runtime=ok check=ok
//! prints: run() => ("d3_another_invocation::first::Counter_Snap<locus::Nat>", "locus::Nat", "locus::Seq<locus::Nat>", "locus::Nat")
//! Required case 3, another invocation: a type, its declared model, and
//! generic functions in one `locus!`; snapshots taken, passed and read in
//! another, which sees none of the declarations' tokens.
use ::locus::locus;
mod first {
    use ::locus::locus;
    locus! {
        pub struct Counter { pub hits: u32 }
        pub struct Stack { pub items: Vec<u32> }
        impl Model for Stack {
            type Logic = Seq<Nat>;
            logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
        }
        pub fn keep<T: Model>(x: &T) -> Snap<T> { logic! { snap(x) } }
        pub logic fn hits_of(c: Snap<Counter>) -> Nat { c.hits }
    }
}
locus! {
    use first::{Counter, Stack, keep, hits_of};
    fn pass<T: Model>(x: &T) -> Snap<T> { keep(x) }
    logic fn takes_nat(n: Nat) -> Nat { n }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str) {
        let c = Counter { hits: 3 };
        let s = Stack { items: vec![1] };
        let kept = pass(&c);
        let read = logic! { takes_nat(hits_of(kept)) + takes_nat(kept.hits) + takes_nat(c.hits) };
        (type_of(&kept), type_of(&read), type_of(&logic! { s }), type_of(&logic! { Stack::model(snap(s)).len() }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
