//! expect: runtime=ok check=ok
//! prints: stub of Int: every stub | stub of Rational: every stub | marker: the marker | the library's trait at the program's stub: 0
//! Plain Rust, written by hand: an arrangement that avoids s08's collision
//! from syntax alone, and that this spike's lowering does not use. Every
//! stub is `Box<Tag>`, a wrapper that the orphan rule sees through, over a
//! tag of its own. An implementation for every logical type is written over
//! `Box<X>`, which the marker is not; and the library's trait can still be
//! implemented for a program's stub.
use ::locus::{LogicalData, Proj};

// What the library would have.
trait Tag {}
struct IntTag;
impl Tag for IntTag {}
type Stub<X> = Box<X>;

// The program.
trait Named {
    fn name() -> &'static str;
}
struct RationalTag;
impl Tag for RationalTag {}
impl<X: Tag> Named for Stub<X> {
    fn name() -> &'static str {
        "every stub"
    }
}
impl Named for LogicalData {
    fn name() -> &'static str {
        "the marker"
    }
}
// A trait of the library, for a stub of the program.
impl Proj for Stub<RationalTag> {
    type Out = ();
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!(
        "stub of Int: {} | stub of Rational: {} | marker: {} | the library's trait at the program's stub: {}",
        <Stub<IntTag> as Named>::name(),
        <Stub<RationalTag> as Named>::name(),
        <LogicalData as Named>::name(),
        std::mem::size_of::<<Stub<RationalTag> as Proj>::Out>()
    );
}
