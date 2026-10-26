// The old design's checking program: the ghost binding kept, as Erased.
use token_sensitive::token_sensitive;

#[token_sensitive]
fn answer() -> u32 {
    let x = 0;
    let h: ::__locus::Erased = { ::__locus::observe(&x); ::__locus::Erased };
    x
}

fn main() {
    println!("answer() = {}", answer());
}
