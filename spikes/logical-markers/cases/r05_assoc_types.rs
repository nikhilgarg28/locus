// expect: E0119
// One marker: an associated type cannot differ between two logical types.
extern crate lu;
use lu::*;
trait Family { type Out; }
impl Family for Int { type Out = Int; }
impl Family for Nat { type Out = Erased; }
fn main() {}
