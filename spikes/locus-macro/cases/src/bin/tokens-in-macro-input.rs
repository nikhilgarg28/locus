//! Spike case for question 2: `litmus/compatibility/tokens-in-macro-input.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Are tokens that look like Locus syntax left alone inside a macro's input?
//~ spec: compat.syntax.not-rust
//~ rfc: 0001

macro_rules! pair {
    ($a:ident @ $b:ident) => {
        ($a, $b)
    };
}

fn shown() -> &'static str {
    stringify!(@(x > 0))
}

fn joined(x: u32, y: u32) -> (u32, u32) {
    pair!(x @ y)
}

//~ run: shown() => "@(x > 0)"
//~ run: joined(1, 2) => (1, 2)

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"shown()"#, shown());
    println!("{} => {:?}", r#"joined(1, 2)"#, joined(1, 2));
}
