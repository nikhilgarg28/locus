//! expect: runtime=E0277 check=E0277
//! A field that holds a trait object, with no mark: an error at the
//! declaration, in the default form.
use ::locus::locus;
locus! {
    trait Shape { fn area(&self) -> u32; }
    struct Scene { shapes: Vec<Box<dyn Shape>>, count: u32 }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
