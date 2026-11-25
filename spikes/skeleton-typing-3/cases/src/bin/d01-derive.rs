//! expect: runtime=ok check=ok
//! prints: sizes: Rational 0, Tree 0, Pair<Tree> 0, List<Rational> 0, Tagged 4
//! prints: debug: Rational Tree Pair List
//! prints: build() => 7
//! `#[derive(Logical)]` types: one with no parameters, a generic one, and
//! two that contain themselves. Each is a zero-sized Rust type of its own,
//! under its own name, with `Clone`, `Copy`, `Debug` and `Logical`.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    struct Rational { num: Int, den: Int }

    #[derive(Logical)]
    enum Tree {
        Leaf,
        Node(Tree, Nat, Tree),
    }

    #[derive(Logical)]
    struct Pair<T> { first: T, second: T }

    #[derive(Logical)]
    enum List<T> {
        Nil,
        Cons(T, List<T>),
    }

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

    impl<T> Pair<T> {
        logic fn swap(p: Pair<T>) -> Pair<T> {
            Pair { first: p.second, second: p.first }
        }
    }

    // A runtime type with a logical field: the field costs nothing.
    struct Tagged {
        n: u32,
        shape: Pair<Tree>,
    }

    // A logic function in a module, called by its path.
    mod geometry {
        pub logic fn area(side: Nat) -> Nat { side * side }
    }

    fn keep<G: Logical>(g: G) -> G { g }
    fn takes_tree(t: Tree) -> u32 { 1 }
    fn takes_pair(p: Pair<Tree>, q: Pair<Tree>) -> u32 { 2 }
    fn takes_list(xs: List<Rational>, n: Nat) -> u32 { 4 }

    fn build() -> u32 {
        let leaf = logic! { Tree::Leaf };
        let both = logic! { Pair { first: leaf, second: Tree::Node(leaf, 0, leaf) } };
        let swapped: Pair<Tree> = logic! { Pair::swap(both) };
        let halves = logic! { List::Cons(Rational { num: 1, den: 2 }, List::Nil) };
        let n = logic! { geometry::area(length(halves)) };
        let tagged = Tagged { n: 0, shape: both };
        // `both` is used twice: the marker is `Copy` though `Tree` is not asked to be.
        tagged.n + takes_tree(keep(leaf)) + takes_pair(both, swapped) + takes_list(halves, n)
    }

    fn debug() -> String {
        let r: Rational = logic! { Rational { num: 1, den: 2 } };
        let t: Tree = logic! { Tree::Leaf };
        let p: Pair<Nat> = logic! { Pair { first: 1, second: 2 } };
        let l: List<Seq<Int>> = logic! { List::Nil };
        format!("{:?} {:?} {:?} {:?}", r, t, p, l)
    }
}

fn main() {
    use std::mem::size_of;
    println!("expansion: {}", ::locus::__expansion!());
    println!(
        "sizes: Rational {}, Tree {}, Pair<Tree> {}, List<Rational> {}, Tagged {}",
        size_of::<Rational>(),
        size_of::<Tree>(),
        size_of::<Pair<Tree>>(),
        size_of::<List<Rational>>(),
        size_of::<Tagged>()
    );
    println!("debug: {}", debug());
    println!("build() => {:?}", build());
}
