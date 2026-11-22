// Probe 4: two implementations for two members of a type family, in the crate
// that declares the family. (In another crate it is E0119: skel/c07.)
#![allow(dead_code)]
pub struct A; pub struct B;
pub trait Snapshot { type S; }
impl Snapshot for u32 { type S = A; }
impl Snapshot for u8 { type S = B; }
pub trait Describe { fn width() -> u32; }
impl Describe for <u32 as Snapshot>::S { fn width() -> u32 { 32 } }
impl Describe for <u8 as Snapshot>::S { fn width() -> u32 { 8 } }
fn main() { println!("{} {}", <A as Describe>::width(), <B as Describe>::width()); }
