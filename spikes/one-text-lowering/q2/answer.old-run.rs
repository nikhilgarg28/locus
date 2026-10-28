// The old design's runtime program: the ghost binding removed.
use token_sensitive::token_sensitive;

#[token_sensitive]
fn answer() -> u32 {
    let x = 0;

    x
}

fn main() {
    println!("answer() = {}", answer());
}
