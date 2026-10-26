//! Plain Rust that depends on a Locus library, so it is checked too.
fn main() {
    let n = 10;
    let _h = b::nonzero!(n);
    let m = b::Meters(3);
    println!("half {} quadruple {} answer {}", b::half(n), b::quadruple(&m), b::answer());
    #[cfg(feature = "outside")]
    println!("triple {}", outside::triple(n, b::nonzero!(n)));
}

#[cfg(test)]
mod tests {
    #[test]
    fn half() {
        let n = 4;
        let _h = b::nonzero!(n);
        assert_eq!(b::half(n), 2);
    }
}
