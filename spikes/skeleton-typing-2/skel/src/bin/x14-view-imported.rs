//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat")
//! A logical enum imported by name from another module, and matched there.
//! `use shapes::Tree;` does not import the view that the patterns are
//! rewritten to name. A path through the module does reach it.
use ::locus::locus;

locus! {
    mod shapes {
        #[derive(Logical)]
        pub enum Tree { Leaf, Node(Tree, Nat, Tree) }
    }
    mod by_path {
        pub logic fn root(t: super::shapes::Tree) -> Nat {
            match t {
                super::shapes::Tree::Leaf => 0,
                super::shapes::Tree::Node(_, n, _) => n,
            }
        }
    }
    mod by_name {
        use super::shapes::Tree;
        pub logic fn root(t: Tree) -> Nat {
            match t {
                Tree::Leaf => 0,
                Tree::Node(_, n, _) => n,
            }
        }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(t: shapes::Tree) -> (&'static str, &'static str) {
        let a = logic! { by_path::root(t) };
        let b = logic! { by_name::root(t) };
        (type_of(&a), type_of(&b))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
