//! expect: runtime=ok check=ok
//! prints: run() => "locus::Seq<locus::BoundedU32>"
//! A generic function that `locus!` did not lower (plain Rust, here
//! outside the macro) calls a lowered one that takes a snapshot of its
//! parameter. The lowered function has the bound; the caller cannot.
use ::locus::locus;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    pub fn kept<T>(x: Vec<T>) -> &'static str { type_of(&logic! { snap(x) }) }
}
fn plain_rust<T>(x: Vec<T>) -> &'static str { kept(x) }
fn run() -> &'static str { plain_rust(vec![1u32]) }
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
