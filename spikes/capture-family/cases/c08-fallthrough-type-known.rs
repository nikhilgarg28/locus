// expect: ok. The element type is known and has no model, so the call falls through `Deref` to the standard method.
include!("../lib.rs");
use std::ops::{Deref, DerefMut};
struct LVec<T> { inner: Vec<T> }
impl<T> Deref for LVec<T> { type Target = Vec<T>; fn deref(&self) -> &Vec<T> { &self.inner } }
impl<T> DerefMut for LVec<T> { fn deref_mut(&mut self) -> &mut Vec<T> { &mut self.inner } }
impl<T: Model> LVec<T> { fn push(&mut self, x: T) -> Proof { self.inner.push(x); Proof } }
fn main() {
    let mut a: LVec<u32> = LVec { inner: vec![1u32] };
    let _p: Proof = a.push(2);
    let mut b: LVec<std::net::SocketAddr> = LVec { inner: Vec::new() };
    let unit: () = b.push("127.0.0.1:1".parse().unwrap());
    println!("{} {} {:?}", a.len(), b.len(), unit);
}
