//! An integration test: plain Rust, checked because it depends on `b`.
#[test]
fn quadruple() {
    let m = b::Meters(2);
    let _seen = b::nonzero!(m.0);
    assert_eq!(b::quadruple(&m), 8);
}
