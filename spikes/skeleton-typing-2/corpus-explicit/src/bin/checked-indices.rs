//! Spike case for question 2: `litmus/contracts/checked-indices.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: May a vector hold indices, each with a proof that it is in bounds?
//~ spec: logic.ghost.runtime-parts-kept
//~ rfc: 0002

fn in_bounds(items: &[u32], wanted: &[usize]) -> Vec<(i: usize, @(i < items.len()))> {
    let mut checked: Vec<(i: usize, @(i < items.len()))> = Vec::new();
    for &i in wanted {
        if i < items.len() {
            checked.push((i, _));
        }
    }
    checked
}

//~ run: in_bounds(&[1, 2, 3], &[0, 5, 2]).len() => 2

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!(
        "{} => {:?}",
        r#"in_bounds(&[1, 2, 3], &[0, 5, 2]).len()"#,
        in_bounds(&[1, 2, 3], &[0, 5, 2]).len()
    );
}
