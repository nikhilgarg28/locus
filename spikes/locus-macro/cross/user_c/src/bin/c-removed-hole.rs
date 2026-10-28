//! Question 5: a Locus crate calling `divide` from inside its own `locus!`. Strategy (ii), with a hole, written as for (i).
use locus::locus;

locus! {
    fn main() {
        let q = lib_a_removed::divide(10, 2, _);
        println!("{q:?}");
    }
}
