// Probe 6: can a pattern reach a view's variants through the logical type,
// by an associated type? (No: E0223, an ambiguous associated type.)
#![allow(dead_code)]
pub enum View { Leaf, Node(u8) }
pub struct Tree;
pub trait HasView { type View; }
impl HasView for Tree { type View = View; }
fn root(v: View) -> u8 {
    match v {
        <Tree as HasView>::View::Leaf => 0,
        <Tree as HasView>::View::Node(n) => n,
    }
}
fn main() {}
