//! expect: runtime=reject check=reject
//! An implementation written for `Snap<u32>` is not the one rustc selects
//! for `Snap<bool>`: there is none for it. With one marker for every
//! snapshot, rustc selected it (h03 of the first spike), and the first
//! mode had to reject the header.
use ::locus::locus;

locus! {
    #![spike(allow_ghost_headers)]
    trait Describe: Logical { type Width: Logical; }
    impl Describe for Snap<u32> { type Width = Nat; }
}

fn main() {
    use std::any::type_name;
    println!("expansion: {}", ::locus::__expansion!());
    println!("at Snap<u32>: {}", type_name::<<::locus::Snap<u32> as Describe>::Width>());
    println!("at Snap<bool>: {}", type_name::<<::locus::Snap<bool> as Describe>::Width>());
}
