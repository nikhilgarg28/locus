//! Question 5: a Locus crate calling `divide` from inside its own `locus!`. Strategy (i), with a hole.
use locus::locus;

locus! {
    fn main() {
        let q = lib_a_marker::divide(10, 2, _);
        println!("{q:?}");
    }
}
