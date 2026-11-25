// expect: E0499
// A snapshot whose type keeps the runtime type keeps its lifetimes, and so keeps a borrow alive.
extern crate lg;
use lg::*;
fn main() {
    let mut v = vec![1u32];
    let r = &mut v;
    let before = snap_typed(&r);     // ghost: the snapshot of `r`
    r.push(2);
    v.push(3);                       // fine in Rust: `r` is not used again
    use_ghost(before);               // a later proof mentions the snapshot
}
