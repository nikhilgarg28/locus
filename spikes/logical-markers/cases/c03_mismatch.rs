// expect: E0308
// Today rustc accepts this, since both are `Vec<LogicalData>`, and Locus's own typing rejects it.
extern crate lg;
use lg::*;
fn main() {
    let v: Vec<Nat> = Vec::new();
    let _w: Vec<Seq<Int>> = v;
}
