//! Question 5: a Locus crate calling `divide` from inside its own `locus!`. Strategy (ii), with a proof from a runtime check, which is not a hole, so the toy keeps it.
use locus::locus;

locus! {
    #![spike(drop_hole_args)]

    fn main() {
        let q = lib_a_removed::divide(10, 2, lib_a_removed::check_nonzero(2));
        println!("{q:?}");
    }
}
