//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Seq<locus::Nat>", "h1_derive_model::Point_Logic", "locus::Nat", "h1_derive_model::Pair_Logic<locus::Nat, locus::Seq<locus::Int>>", "locus::Seq<locus::Int>", "h1_derive_model::List_Logic", "locus::Nat")
//! Round 2's d5, for round 3's rules: `impl Model` and `#[derive(Model)]`, for a struct, a generic struct
//! and an enum that contains itself through `Box`. `model(x)` is typed
//! through the family `Model::Logic`, as a snapshot is through `Snap`.
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32> }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(stack: Snap<Stack>) -> Seq<Nat> { stack.items }
    }
    #[derive(Model, Clone)]
    struct Point { x: u32, y: u32, positive: @(x > 0) }
    #[derive(Model)]
    struct Pair<'a, A, B> { first: A, second: &'a B }
    #[derive(Model)]
    enum List { Nil, Cons(u32, Box<List>) }

    logic fn norm(p: Point_Logic) -> Nat { p.x + p.y }
    logic fn length(l: List_Logic) -> Nat {
        match l { List_Logic::Nil => 0 as Nat, List_Logic::Cons(_, rest) => length(rest) + 1 }
    }
    /// Generic over a type that has a model.
    fn first_of<A: Model, B: Model>(p: &Pair<'_, A, B>) -> u32 {
        let first = logic! { p.first };
        1
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let s = Stack { items: vec![1] };
        let p = Point { x: 1, y: 2, positive: _ };
        let ints = vec![1i32];
        let pair = Pair { first: 3u8, second: &ints };
        let l = List::Cons(1, Box::new(List::Nil));
        let _ = first_of(&pair);
        (type_of(&logic! { s }), type_of(&logic! { p }), type_of(&logic! { norm(p) }),
         type_of(&logic! { pair }), type_of(&logic! { pair.second }),
         type_of(&logic! { l }), type_of(&logic! { length(l) }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
