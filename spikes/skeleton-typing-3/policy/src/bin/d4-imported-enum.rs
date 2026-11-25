//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat", "locus::Nat")
//! A logical enum imported by name from another module, a generic one, and
//! one reached through an alias of the module: the patterns reach the
//! variants through the type alone, so nothing else has to be imported.
use ::locus::locus;
locus! {
    mod shapes {
        #[derive(Logical)]
        pub enum Tree { Leaf, Node(Tree, Nat, Tree) }
        #[derive(Logical)]
        pub enum Maybe<T> { Nothing, Just(T) }
    }
    mod user {
        use super::shapes::Tree;
        use super::shapes::Maybe;
        use super::shapes as s;
        pub logic fn size(t: Tree) -> Nat {
            match t { Tree::Leaf => 0 as Nat, Tree::Node(l, _, r) => size(l) + 1 + size(r) }
        }
        pub logic fn or_zero(m: Maybe<Nat>) -> Nat {
            match m { Maybe::Nothing => 0 as Nat, Maybe::Just(n) => n }
        }
        pub logic fn through_a_path(t: s::Tree) -> Nat {
            match t { s::Tree::Leaf => 1 as Nat, s::Tree::Node(..) => 2 as Nat }
        }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let t = logic! { shapes::Tree::Leaf };
        (type_of(&logic! { user::size(t) }), type_of(&logic! { user::or_zero(shapes::Maybe::Just(1 as Nat)) }),
         type_of(&logic! { user::through_a_path(t) }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
