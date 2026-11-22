// expect: E0277
// `Logical` as a bound keeps runtime types out.
extern crate lg;
use lg::*;
fn only_ghost<G: Logical + Describe>() -> &'static str { G::describe() }
fn main() { only_ghost::<u32>(); }
