//! expect: runtime=ok check=ok
//! prints: at Snap<u32>: locus::Nat
//! prints: at Snap<&mut Box<u32>>: locus::Nat
//! c03 without the use at `Snap<bool>`: the implementation is selected at
//! its own snapshot type, a reference's and a `Box`'s being their target's.
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
    println!(
        "at Snap<&mut Box<u32>>: {}",
        type_name::<<::locus::Snap<&mut Box<u32>> as Describe>::Width>()
    );
}
