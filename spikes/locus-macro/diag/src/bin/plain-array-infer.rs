//! Question 4: `[0; _]` from `underscore-in-rust-positions`, as plain Rust
//! outside any `locus!` block, to see whether rust-analyzer infers it there.
fn zeros() -> usize {
    let xs: [u8; 4] = [0; _];
    xs.len()
}

fn main() {
    println!("{}", zeros());
}
