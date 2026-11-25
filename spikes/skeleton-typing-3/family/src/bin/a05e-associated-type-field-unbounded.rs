//! expect: runtime=E0277 check=E0277
//! As a05 without the bound: rustc's message names the associated type.
use ::locus::locus;
locus! {
    trait Family { type Out; }
    struct Kept<F: Family> { out: F::Out, n: u32 }
    fn kept<F: Family>(k: &Kept<F>) -> u32 { let s = logic! { snap(k) }; 1 }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
