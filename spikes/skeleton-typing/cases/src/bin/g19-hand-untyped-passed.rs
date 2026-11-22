//! expect: runtime=E0308 check=E0308
//! Plain Rust, written by hand: what a wrong pick would give. If lowering
//! took a ghost local for one that only logical code mentions, and runtime
//! code passes it where a `Nat` is wanted, rustc rejects the untyped marker.
//! (g15 is the wrong pick that rustc does not reject: its use is generic.)
use ::locus::{Nat, Untyped};

fn takes_nat(_n: Nat) -> u32 {
    1
}

fn run() -> u32 {
    let before = Untyped; // let before = logic! { .. };
    takes_nat(before)
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
