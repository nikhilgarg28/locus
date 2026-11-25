// expect: ok
// The same program with one marker for every snapshot: no lifetime, no borrow.
extern crate lg;
use lg::*;
fn main() {
    let mut v = vec![1u32];
    let r = &mut v;
    let before = snap_uniform(&r);
    r.push(2);
    v.push(3);
    use_ghost(before);
    println!("accepted, v = {:?}", v);
}
