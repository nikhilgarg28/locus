//! Spike case for question 2: `litmus/logical-layer/logical-fields.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May a logical type have only logical fields, payloads, and type arguments?
//~ spec: logic.types.contents-logical, logic.types.positive
//~ rfc: 0002

#[derive(Logical)]
enum Tree {
    Leaf,
    Node(Tree, Nat, Tree),
}

#[derive(Logical)]
struct Span {
    start: Nat,
    length: Nat,
}

logic fn size(t: Tree) -> Nat {
    match t {
        Tree::Leaf => 0,
        Tree::Node(left, _, right) => size(left) + 1 + size(right),
    }
}

logic fn count(xs: Seq<Nat>) -> Nat {
    xs.len()
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
