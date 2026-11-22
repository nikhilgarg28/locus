// expect: E0119
// One marker (today's lowering): two logical types are one Rust type, so two implementations conflict.
extern crate lu;
use lu::*;
trait Size { fn size() -> u32; }
impl Size for Int { fn size() -> u32 { 1 } }
impl Size for Nat { fn size() -> u32 { 2 } }
fn main() {}
