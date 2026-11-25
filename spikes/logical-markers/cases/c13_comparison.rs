// expect: E0308
// A comparison cannot: Rust fixes the result of `<` as the runtime `bool`, and a ghost `Bool` is wanted.
extern crate lg;
use lg::*;
use std::cmp::Ordering;
#[derive(Clone, Copy)] struct Ver;
impl PartialEq for Ver { fn eq(&self, _: &Ver) -> bool { true } }
impl PartialOrd for Ver { fn partial_cmp(&self, _: &Ver) -> Option<Ordering> { None } }
fn main() {
    let lt: Bool = Ver < Ver;
    let _ = lt;
}
