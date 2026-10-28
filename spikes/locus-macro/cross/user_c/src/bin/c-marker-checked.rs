//! Question 5: a Locus crate calling `divide` from inside its own `locus!`. Strategy (i), with a proof from a runtime check.
use locus::locus;

locus! {
    fn main() {
        let q = lib_a_marker::divide(10, 2, lib_a_marker::check_nonzero(2));
        println!("{q:?}");
    }
}
