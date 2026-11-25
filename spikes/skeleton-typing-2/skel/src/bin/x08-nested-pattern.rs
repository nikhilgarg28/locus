//! expect: runtime=ok check=ok
//! prints: run() => "locus::Nat"
//! A variant pattern inside a variant pattern. The payload of a view's
//! variant is the logical enum itself, a struct with no variants for
//! rustc, so this `match` has no skeleton.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    enum Tree { Leaf, Node(Tree, Nat, Tree) }
    logic fn left_most(t: Tree) -> Nat {
        match t {
            Tree::Leaf => 0,
            Tree::Node(Tree::Leaf, n, _) => n,
            Tree::Node(rest, _, _) => left_most(rest),
        }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(t: Tree) -> &'static str {
        let n = logic! { left_most(t) };
        type_of(&n)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
