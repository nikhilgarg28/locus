//! Question 5: plain Rust, no `locus!`. Passes a marker to strategy (ii)'s `divide`.
fn main() {
    println!("{}", lib_a_removed::divide(10, 2, locus::Erased));
}
