//! expect: runtime=E0200 check=E0200
//! A crate that is NOT lowered: plain Rust, with no `locus!` in it. A
//! hand-written implementation of `Model` for a type of its own, without
//! `unsafe`: rustc refuses it.
struct Mine {
    #[allow(dead_code)]
    n: u32,
}
impl locus::Model for Mine {
    type Snap = locus::Nat;
    type Logic = locus::Seq<locus::Nat>;
    fn __logic_model(_: locus::Nat) -> locus::Seq<locus::Nat> {
        locus::stub()
    }
    fn __logic_valid(_: locus::Nat) -> locus::Prop {
        locus::True
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
