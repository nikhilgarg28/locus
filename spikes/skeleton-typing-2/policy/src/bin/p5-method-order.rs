//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Int", "locus::Nat", "locus::Int", "locus::Int", "locus::Nat", "locus::Nat")
//! P5: a method call on a runtime place finds a logic function declared on
//! the runtime type first, then one on its model. `Stack` declares
//! `room`, and a `len` of its own that gives an `Int`; its model, a
//! `Seq`, has `len` and `last`. `Plain` has no model.
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32>, cap: usize }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: &Stack) -> Seq<Nat> { s.items }
    }
    impl Stack {
        logic fn room(self) -> Int { self.cap - self.items.len() }
        logic fn len(self) -> Int { self.items.len() as Int }
    }
    struct Plain { n: u32 }
    impl Plain {
        logic fn twice(self) -> Nat { self.n + self.n }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let s = Stack { items: vec![1, 2], cap: 4 };
        let by_ref = &s;
        let boxed = Box::new(Stack { items: vec![], cap: 1 });
        let p = Plain { n: 2 };
        let on_the_type = logic! { s.room() };
        let on_the_model = logic! { s.last() };
        let the_types_len = logic! { s.len() };
        let through_a_reference = logic! { by_ref.room() + boxed.len() };
        let the_models_len = logic! { model(s).len() };
        let no_model = logic! { p.twice() };
        (type_of(&on_the_type), type_of(&on_the_model), type_of(&the_types_len),
         type_of(&through_a_reference), type_of(&the_models_len), type_of(&no_model))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
