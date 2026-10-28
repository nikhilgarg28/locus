//! Question 5, strategy (ii): the proof parameter exists only in the checking
//! expansion; the runtime `divide` takes two arguments.
use locus::locus;

locus! {
    #![spike(strip_proof_params)]

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
