//! Spike case for question 2: `litmus/contracts/final-value-at-return.accept.lc`, its full text copied
//! unchanged inside `locus! { ... }`. Written by `gen-cases.py`; `run.sh`
//! checks that the copy is still exact. Line N of the example is line N + 7 here.
//! `::locus` rather than `locus`: see `x-locus-name-collision.rs`.
use ::locus::locus;

locus! {
//~ question: Does a `&mut` parameter mean its entry value in a signature, and `final!` its value at return?
//~ spec: contracts.entry-values, contracts.final
//~ rfc: 0002

fn bump(counter: &mut u32, room: @(counter < 100)) -> @(final!(counter) == counter + 1) {
    *counter += 1;
    _
}

fn six() -> u32 {
    let mut n: u32 = 5;
    let grew = bump(&mut n, _);
    let is_six: @(n == 6) = _;
    n
}

//~ run: six() => 6

}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("{} => {:?}", r#"six()"#, six());
}
