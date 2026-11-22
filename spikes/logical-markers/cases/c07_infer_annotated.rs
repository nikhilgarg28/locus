// expect: ok
// The same, with the annotation that RFC 0002 does not require today.
extern crate lg;
use lg::*;
fn main() {
    let before: Nat = ghost();
    let _ = &before;
    println!("annotated: accepted");
}
