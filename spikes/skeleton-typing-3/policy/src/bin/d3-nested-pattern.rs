//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat", "locus::Nat")
//! A variant pattern inside a variant pattern, two deep, in a `match` on
//! a logical enum, with a generic one inside.
use ::locus::locus;
locus! {
    #[derive(Logical)]
    enum Maybe<T> { Nothing, Just(T) }
    #[derive(Logical)]
    enum Tree { Leaf, Node(Tree, Nat, Tree), Tagged(Maybe<Nat>, Tree) }
    logic fn left_value(t: Tree) -> Nat {
        match t {
            Tree::Node(Tree::Node(_, n, _), _, _) => n,
            Tree::Node(Tree::Leaf, n, _) => n,
            Tree::Node(Tree::Tagged(Maybe::Just(tag), _), n, Tree::Leaf) => tag + n,
            Tree::Tagged(Maybe::Just(tag), _) => tag,
            _ => 0 as Nat,
        }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let t = logic! { Tree::Node(Tree::Leaf, 3 as Nat, Tree::Leaf) };
        let v = logic! { left_value(t) };
        let m = logic! { match Maybe::Just(v) { Maybe::Just(x) => x, Maybe::Nothing => v } };
        let direct = logic! { match t { Tree::Tagged(Maybe::Nothing, Tree::Leaf) => v, _ => m } };
        (type_of(&v), type_of(&m), type_of(&direct))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
