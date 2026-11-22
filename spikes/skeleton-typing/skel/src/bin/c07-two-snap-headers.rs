//! expect: runtime=ok check=ok
//! prints: widths: locus::Nat locus::Int
//! Two implementations of one trait, for `Snap<u32>` and for `Snap<u8>`,
//! which are two types. In a crate that does not declare the family,
//! rustc does not normalize a projection in an `impl` header when it
//! looks for overlap, so each of them overlaps every other.
use ::locus::locus;

locus! {
    #![spike(allow_ghost_headers)]
    trait Describe: Logical { type Width: Logical; }
    impl Describe for Snap<u32> { type Width = Nat; }
    impl Describe for Snap<u8> { type Width = Int; }
}

fn main() {
    use std::any::type_name;
    println!("expansion: {}", ::locus::__expansion!());
    println!(
        "widths: {} {}",
        type_name::<<::locus::Snap<u32> as Describe>::Width>(),
        type_name::<<::locus::Snap<u8> as Describe>::Width>()
    );
}
