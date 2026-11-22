//! expect: runtime=E0277 check=E0277
//! Trait objects read in logical code: neither a trait declared in `locus!`
//! nor `dyn Debug` has a model. Lowering writes none for a trait.
use ::locus::locus;
locus! {
    trait Shape { fn area(&self) -> u32; }
    fn read(shape: Box<dyn Shape>, note: &dyn std::fmt::Debug) -> u32 {
        let a = logic! { snap(shape) };
        let b = logic! { snap(note) };
        1
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
