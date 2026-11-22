//! expect: runtime=E0119 check=E0119
//! Plain Rust, written by hand: s08 with the program's implementation
//! bounded by an associated type that is one type at every stub and
//! another at the marker. rustc does not use that to tell them apart.
use ::locus::{__Kind, __StubKind, Logical, LogicalData};

trait Named {}
impl<T: Logical + __Kind<K = __StubKind>> Named for T {}
impl Named for LogicalData {}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
