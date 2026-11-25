//! expect: runtime=reject check=reject
//! A `match` in logical code that leaves a variant out. Locus rejects it,
//! and so does rustc, on the view: E0004.
use ::locus::locus;

locus! {
    #[derive(Logical)]
    enum Tree { Leaf, Node(Tree, Nat, Tree) }
    logic fn root(t: Tree) -> Nat {
        match t {
            Tree::Node(_, n, _) => n,
        }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
