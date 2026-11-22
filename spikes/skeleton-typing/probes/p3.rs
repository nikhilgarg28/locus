// Probe 3: a recursive zero-sized type with no indirection.
pub struct Nat;
pub enum Tree { Leaf, Node(Tree, Nat, Tree) }
pub struct A { a: [A; 0] }
fn main() {}
