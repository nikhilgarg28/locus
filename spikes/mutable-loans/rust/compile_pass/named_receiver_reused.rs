// Rust accepts this; the proposed Locus acquisition binding rule must reject it.
fn lend(value: &mut i32) -> &mut i32 {
    value
}
fn main() {
    let mut value = 0;
    let r = &mut value;
    let x = lend(r);
    *x = 1;
    *r = 2;
    assert_eq!(value, 2);
}
