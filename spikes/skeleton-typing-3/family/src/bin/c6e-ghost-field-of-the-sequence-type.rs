//! expect: runtime=E0277 check=E0277
//! Why the ghost field's type is nominal. Written as what it holds,
//! `Seq<Snap<T>>`, the field asks for `T: Model` where the struct is declared.
use ::locus::locus;
locus! {
    pub struct Naive<T> {
        inner: std::vec::Vec<T>,
        contents: Seq<Snap<T>>,
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
