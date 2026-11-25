//! expect: runtime=ok check=ok
//! prints: run() => ("a04_match::Tree", "locus::Nat", "locus::Nat", "locus::Nat", 0, 4)
//! `match` on a logical enum: one that contains itself, a generic one, in a
//! logic function and in a `logic!`. The enum is zero-sized, and so a
//! runtime struct with a field of its type pays nothing for it.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    enum Tree { Leaf, Node(Tree, Nat, Tree) }
    #[derive(Logical)]
    enum List<T> { Nil, Cons(T, List<T>) }

    logic fn size(t: Tree) -> Nat {
        match t {
            Tree::Leaf => 0,
            Tree::Node(left, _, right) => size(left) + 1 + size(right),
        }
    }
    logic fn length<T>(xs: List<T>) -> Nat {
        match xs {
            List::Nil => 0,
            List::Cons(_, rest) => 1 + length(rest),
        }
    }
    logic fn head_or<T>(xs: List<T>, otherwise: T) -> T {
        match xs {
            List::Nil => otherwise,
            List::Cons(x, _) => x,
        }
    }
    struct Tagged { n: u32, shape: Tree }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run() -> (&'static str, &'static str, &'static str, &'static str, usize, usize) {
        let t = logic! { Tree::Node(Tree::Leaf, 3, Tree::Leaf) };
        let n = logic! { size(t) };
        let k = logic! { match t { Tree::Leaf => 0, Tree::Node(_, k, _) => k } };
        let h = logic! { head_or(List::Cons(n, List::Nil), k) + length(List::Cons(t, List::Nil)) };
        (type_of(&t), type_of(&n), type_of(&k), type_of(&h), std::mem::size_of_val(&t), std::mem::size_of::<Tagged>())
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
