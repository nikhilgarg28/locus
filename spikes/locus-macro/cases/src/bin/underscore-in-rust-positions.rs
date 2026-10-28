//! Spike case for question 2: `litmus/compatibility/underscore-in-rust-positions.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Does `_` keep its Rust meaning where Rust accepts it, rather than being read as a proof hole?
//~ spec: compat.syntax.not-rust
//~ rfc: 0001

fn first_of(pair: (u32, u32)) -> u32 {
    let a;
    (a, _) = pair;
    a
}

fn zeros() -> usize {
    let xs: [u8; 4] = [0; _];
    let ys = Vec::<_>::from(xs);
    match ys.len() {
        _ => ys.len(),
    }
}

//~ run: first_of((7, 9)) => 7
//~ run: zeros() => 4

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"first_of((7, 9))"#, first_of((7, 9)));
    println!("{} => {:?}", r#"zeros()"#, zeros());
}
