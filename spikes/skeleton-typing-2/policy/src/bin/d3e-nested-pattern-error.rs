//! expect: runtime=reject check=reject
//! Inside a nested pattern rustc still checks the variant's name, its
//! number of fields, and the type of what it binds.
use ::locus::locus;
locus! {
    #[derive(Logical)]
    enum Tree { Leaf, Node(Tree, Nat, Tree) }
    logic fn a(t: Tree) -> Nat { match t { Tree::Node(Tree::Twig, n, _) => n, _ => 0 as Nat } }
    logic fn b(t: Tree) -> Nat { match t { Tree::Node(Tree::Node(n, _), _, _) => n, _ => 0 as Nat } }
    logic fn c(t: Tree) -> Nat { match t { Tree::Node(Tree::Node(l, _, _), _, _) => l, _ => 0 as Nat } }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
