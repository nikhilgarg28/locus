// A vector of `Rc` must not cross threads, and no `unsafe impl` says so:
// the permission is `Send` only when its element type is.
use permissioned_pointers_spike::pvec::PVec;
use std::rc::Rc;

pub fn f() {
    let mut v = PVec::new();
    v.push(Rc::new(1u32));
    std::thread::spawn(move || drop(v));
}
