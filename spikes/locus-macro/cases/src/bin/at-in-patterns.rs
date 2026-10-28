//! Spike case for question 2: `litmus/compatibility/at-in-patterns.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Does `@` in a pattern keep its Rust meaning, rather than being read as a proof?
//~ spec: compat.syntax.not-rust
//~ rfc: 0001

fn bucket(n: u32) -> u32 {
    match n {
        small @ 0..=9 => small,
        other => other / 10,
    }
}

//~ run: bucket(7) => 7
//~ run: bucket(42) => 4

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"bucket(7)"#, bucket(7));
    println!("{} => {:?}", r#"bucket(42)"#, bucket(42));
}
