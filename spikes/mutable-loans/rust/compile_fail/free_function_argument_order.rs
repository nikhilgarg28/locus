// Expected E0502: a naive free-function lowering loses method two-phase behavior.
fn push(values: &mut Vec<usize>, value: usize) {
    values.push(value);
}
fn main() {
    let mut values = vec![1, 2];
    push(&mut values, values.len());
}
