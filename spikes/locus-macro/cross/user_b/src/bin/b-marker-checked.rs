//! Question 5: plain Rust, no `locus!`. Gets the proof from `check_nonzero`, a runtime check.
fn main() {
    println!(
        "{}",
        lib_a_marker::divide(10, 2, lib_a_marker::check_nonzero(2))
    );
}
