//! expect: runtime=E0277 check=E0277
//! No bound is inserted: a type parameter read in logical code with no
//! `T: Model`, in a function.
use ::locus::locus;
locus! {
    fn g<T>(x: &T) -> u32 { let s = logic! { snap(x) }; 1 }
    fn run() -> u32 { g(&1u32) }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
