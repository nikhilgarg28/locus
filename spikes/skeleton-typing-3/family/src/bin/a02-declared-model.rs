//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Seq<locus::Nat>", "a02_declared_model::Stack_Snap<locus::Seq<locus::Nat>, locus::Nat>", "locus::Seq<locus::Nat>", "locus::Seq<locus::Nat>", "locus::Seq<locus::Nat>", "locus::Int")
//! A declared model replaces the default, with one implementation. A
//! wrapper with no model around a field with one: select then capture is the
//! model, capture then project is the field's snapshot, and the model of
//! that snapshot is reached by writing it. A logic function over the
//! snapshot is called by its path, on `snap(s)`.
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32>, cap: usize }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
    }
    impl Stack {
        logic fn room(s: Snap<Stack>) -> Int { s.cap - s.items.len() }
    }
    struct Wrapper { s: Stack, n: u32 }
    logic fn takes_seq(xs: Seq<Nat>) -> Seq<Nat> { xs }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let w = Wrapper { s: Stack { items: vec![1, 2], cap: 4 }, n: 0 };
        let selected = logic! { w.s };
        let whole = logic! { w };
        let projected = logic! { whole.s };
        let by_path = logic! { Stack::model(projected) };
        let by_name = logic! { model::<Stack>(projected) };
        let used = logic! { takes_seq(w.s) };
        let room = logic! { Stack::room(snap(w.s)) };
        (type_of(&selected), type_of(&projected), type_of(&by_path), type_of(&by_name), type_of(&used), type_of(&room))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
