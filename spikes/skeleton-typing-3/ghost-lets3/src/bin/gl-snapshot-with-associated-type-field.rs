//! `litmus/models/snapshot-with-associated-type-field.accept.lc`, its full text copied unchanged inside
//! `locus! { ... }`. Written by `gen-ghost-lets.py`. Line N of the example
//! is line N + 6 here.
use ::locus::locus;

locus! {
//~ question: May the snapshot of a value with an associated-type field be held and compared, though not looked into?
//~ spec: models.snapshots.opaque, logic.eq.snapshots
//~ rfc: 0002

trait Family {
    type Out;
}

struct Holder<F: Family> {
    id: u32,
    item: F::Out,
}

fn kept<F: Family>(h: Holder<F>) -> Holder<F> where F::Out: Model {
    let before = logic! { snap(h) };
    let moved = h;
    let same: @(snap(moved) === before) = _;
    moved
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
