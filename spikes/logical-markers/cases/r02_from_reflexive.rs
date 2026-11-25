// expect: ok
// One marker: `Int::from(nat)` already resolves, to the standard library's `impl<T> From<T> for T`.
extern crate lu;
use lu::*;
fn main() {
    let n: Nat = LogicalData;
    let _i: Int = From::from(n);
    println!("accepted by rustc: the conversion from Nat to Int is the identity on the marker");
}
