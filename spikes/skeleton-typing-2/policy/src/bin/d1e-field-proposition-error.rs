//! expect: runtime=reject check=reject
//! A type error in a field's proposition is rustc's, in both expansions:
//! a `Vec` compared with a number, and a field that does not exist.
use ::locus::locus;
locus! {
    struct Interval {
        lo: u32,
        items: Vec<u32>,
        wrong: @(items <= lo),
        unknown: @(high >= lo),
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
