//! expect: runtime=E0119 check=E0119
//! Plain Rust, written by hand: s08 with the program's implementation
//! bounded by a helper trait of the library that only stubs implement.
//! rustc still sees a conflict: the library might implement its helper for
//! its marker in a later version.
use ::locus::{__Stub, Logical, LogicalData};

trait Named {}
impl<T: Logical + __Stub> Named for T {}
impl Named for LogicalData {}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
