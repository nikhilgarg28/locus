//! Question 5: plain Rust, no `locus!`. Forges the proof by naming the marker.
fn main() {
    println!("{}", lib_a_marker::divide(10, 0, locus::Erased));
}
