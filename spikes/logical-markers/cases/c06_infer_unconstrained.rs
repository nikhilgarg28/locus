// expect: E0283
// The common case: a ghost local that only later logical code mentions. rustc sees no use.
extern crate lg;
use lg::*;
fn main() {
    let before = ghost();            // let before = logic! { v.len() };
    let _ = &before;                 // a later `prove!` names it; that tells rustc nothing
}
