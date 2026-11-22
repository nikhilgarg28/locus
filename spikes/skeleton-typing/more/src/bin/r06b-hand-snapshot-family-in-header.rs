//! expect: runtime=E0119 check=E0119
//! Plain Rust, written by hand: why r06 cannot write the snapshot family in
//! the header of an `impl`. `<u32 as SnapOf>::S` is a stub of the library,
//! and not the marker, but rustc does not normalize it when it checks two
//! implementations for overlap.
use ::locus::{LogicalData, SnapOf};

trait Named {}
impl Named for <u32 as SnapOf>::S {}
impl Named for LogicalData {}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
