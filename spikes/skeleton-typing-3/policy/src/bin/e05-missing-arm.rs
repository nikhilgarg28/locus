//! expect: runtime=reject check=reject
//! A deliberate error in logical code: a `match` on a logical enum that
//! leaves a variant out.
use ::locus::locus;
locus! {
    #[derive(Logical)]
    enum Tree { Leaf, Node(Tree, Nat, Tree) }
    logic fn depth(t: Tree) -> Nat {
        match t {
            Tree::Node(l, _, _) => depth(l) + 1,
        }
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
