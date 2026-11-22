// expect: E0277. The element type is still being inferred at the call, so rustc commits to the method that asks for a model.
include!("../lib.rs");
use std::ops::{Deref, DerefMut};
struct LVec<T> { inner: Vec<T> }
impl<T> Deref for LVec<T> { type Target = Vec<T>; fn deref(&self) -> &Vec<T> { &self.inner } }
impl<T> DerefMut for LVec<T> { fn deref_mut(&mut self) -> &mut Vec<T> { &mut self.inner } }
impl<T: Model> LVec<T> { fn push(&mut self, x: T) -> Proof { self.inner.push(x); Proof } }
fn main() {
    let mut a = LVec { inner: vec![1u32] };
    let p: Proof = a.push(2);                           // Locus's own, with its proof
    let k = 3u32;
    let mut b = LVec { inner: Vec::new() };
    let unit: () = b.push(move |x: u32| x + k);         // the standard library's, with none
    println!("{} {} {:?} {:?}", a.len(), b.len(), std::mem::size_of_val(&p), unit);
}
