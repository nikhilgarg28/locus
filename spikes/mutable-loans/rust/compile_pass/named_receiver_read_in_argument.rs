// Rust accepts a shared read through r in a two-phase reborrow argument.
// A strict SINGLE runtime use of the named acquisition binding rejects it.
fn main() {
    let mut values = vec![1, 2];
    let r = &mut values;
    r.push(r.len());
    assert_eq!(values, vec![1, 2, 2]);
}
