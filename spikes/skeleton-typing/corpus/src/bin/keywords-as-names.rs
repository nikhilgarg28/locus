//! Spike case for question 2: `litmus/compatibility/keywords-as-names.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Do `logic` and `prop` keep their Rust meaning when used as names?
//~ spec: compat.syntax.contextual-keywords, compat.syntax.not-rust
//~ rfc: 0001

#[allow(non_camel_case_types)]
struct logic {
    prop: u32,
}

fn prop(logic: u32) -> logic {
    logic { prop: logic }
}

fn read(value: logic) -> u32 {
    let logic { prop } = value;
    prop
}

//~ run: read(prop(7)) => 7

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"read(prop(7))"#, read(prop(7)));
}
