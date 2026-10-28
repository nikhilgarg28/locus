//! Question 5, strategy (i): the proof parameter stays, as the marker
//! `locus::Erased`, in both expansions.
use locus::locus;

locus! {
    pub fn divide(x: u32, y: u32, nonzero: @(y != 0)) -> u32 {
        x / y
    }

    /// A runtime check that gives the proof.
    pub fn check_nonzero(y: u32) -> @(y != 0) {
        if y == 0 {
            panic!("zero divisor");
        }
        _
    }
}
