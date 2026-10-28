//! Question 3: type errors at positions that `locus!` rewrote: a hole where a
//! runtime value is expected, and a proof parameter returned as a `u32`.
use locus::locus;

locus! {
    fn halve(n: u32, even: @(n % 2 == 0)) -> u32 {
        n / 2
    }

    fn hole_for_runtime() -> u32 {
        halve(_, _)
    }

    fn proof_as_runtime(n: u32, even: @(n % 2 == 0)) -> u32 {
        even
    }
}

fn main() {
    println!(
        "{} {}",
        hole_for_runtime(),
        proof_as_runtime(2, locus::Erased)
    );
}
