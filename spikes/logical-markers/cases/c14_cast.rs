// expect: ok
// Another route: ghost locals have no logical type for rustc, and each use in runtime code converts.
extern crate lg;
use lg::*;
fn main() {
    let before = Untyped;            // let before = logic! { .. };
    takes_nat(cast(before));         // a declared parameter type decides
    let mut v: Vec<Nat> = Vec::new();
    v.push(cast(before));            // so does a type argument that is already known
    println!("accepted: {} ghost element", v.len());
}
