//! expect: runtime=E0277 check=E0277
//! What a bound on the struct costs: Rust asks for it wherever the type is
//! named, in a function that reads nothing, and no closure can be an element.
use ::locus::locus;
locus! {
    pub struct Bounded<T: Model> {
        inner: std::vec::Vec<T>,
        contents: Seq<Snap<T>>,
    }
    fn count<T>(v: &Bounded<T>) -> usize { v.inner.len() }
    fn closures() -> usize {
        let k = 1u32;
        let v = Bounded { inner: vec![move |x: u32| x + k], contents: logic! { Seq::empty() } };
        v.inner.len()
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
