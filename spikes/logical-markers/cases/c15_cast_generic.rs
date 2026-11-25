// expect: E0283
// The conversion has nothing to go on where the use is generic.
extern crate lg;
use lg::*;
fn main() {
    let before = Untyped;
    let kept = keep(cast(before));
    let _ = &kept;
}
