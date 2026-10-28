//! Question 5: a Locus crate calling `divide` from inside its own `locus!`. Strategy (ii), with a hole, which the toy drops from the runtime call.
use locus::locus;

locus! {
    #![spike(drop_hole_args)]

    fn main() {
        let q = lib_a_removed::divide(10, 2, _);
        println!("{q:?}");
    }
}
