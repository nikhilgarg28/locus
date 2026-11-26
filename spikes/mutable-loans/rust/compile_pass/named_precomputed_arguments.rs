fn main() {
    let mut values = vec![1, 2];
    let n = values.len();
    let r = &mut values;
    r.push(n);
    assert_eq!(values, vec![1, 2, 2]);
}
