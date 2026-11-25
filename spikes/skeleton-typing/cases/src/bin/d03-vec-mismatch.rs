//! expect: runtime=E0308 check=E0308
//! A `Vec<Nat>` used as a `Vec<Seq<Int>>`. With one marker both were
//! `Vec<LogicalData>`, rustc accepted, and only Locus's own typing rejected.
use ::locus::locus;

locus! {
    fn mismatch() -> usize {
        let v: Vec<Nat> = Vec::new();
        let w: Vec<Seq<Int>> = v;
        w.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("mismatch() => {:?}", mismatch());
}
