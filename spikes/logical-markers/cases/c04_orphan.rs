// expect: E0117
// The orphan rule applies as in Rust: the library's trait for the library's type, from outside.
extern crate lg;
use lg::*;
impl OrdLogic for Bool {}
fn main() {}
