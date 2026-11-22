//! expect: runtime=reject check=reject
//! P1, strictly: a literal that is an argument is an `Int`, so where the
//! parameter is a `Nat` its type is written, `3 as Nat`. RFC 0002 makes
//! it a `Nat` there. With LOCUS_SPIKE_LITERALS=expected this compiles.
use ::locus::locus;
locus! {
    logic fn double(n: Nat) -> Nat { n + n }
    fn run() -> u32 {
        let d = logic! { double(3) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
