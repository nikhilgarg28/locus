//! Question 5: plain Rust, no `locus!`. Reuses a proof of `2 != 0` as one of `0 != 0`.
fn main() {
    println!(
        "{}",
        lib_a_marker::divide(10, 0, lib_a_marker::check_nonzero(2))
    );
}
