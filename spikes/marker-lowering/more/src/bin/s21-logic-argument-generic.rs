//! expect: runtime=ok check=ok
//! prints: run() => 1
//! A `logic!` passed straight to a generic function. With one marker in
//! program text there is nothing to infer. With a type for each logical
//! type there is, and no `let` to write it on.
use ::locus::locus;

locus! {
    fn keep<G: Logical>(g: G) -> u32 { 1 }

    fn run() -> u32 { keep(logic! { 5 }) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
