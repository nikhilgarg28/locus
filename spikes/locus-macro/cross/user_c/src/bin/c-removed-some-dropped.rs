//! Question 5: a Locus crate calling `divide` from inside its own `locus!`. The hole-dropping rule applied to a hole that is not a proof argument of `divide`: `Some(_)`.
use locus::locus;

locus! {
    #![spike(drop_hole_args)]

    fn main() {
        let q = Some(_);
        println!("{q:?}");
    }
}
