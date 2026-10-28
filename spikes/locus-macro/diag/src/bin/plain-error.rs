//! Question 3: a plain Rust type error inside a `locus!` block.
use locus::locus;

locus! {
    fn halve(n: u32, even: @(n % 2 == 0)) -> u32 {
        n / 2
    }

    fn label() -> u32 {
        let four: u32 = "four";
        halve(four, _)
    }
}

fn main() {
    println!("{}", label());
}
