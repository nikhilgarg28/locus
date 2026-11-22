//! Spike case for question 2: `litmus/logical-layer/same-is-a-proposition.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Is `===` a proposition that a logical type has without declaring it?
//~ spec: logic.eq.same
//~ rfc: 0002

#[derive(Logical)]
enum Light {
    Red,
    Green,
}

logic fn red_is_red() -> @(Light::Red === Light::Red) {
    _
}

logic fn red_is_not_green() -> @(Light::Red !== Light::Green) {
    _
}

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
