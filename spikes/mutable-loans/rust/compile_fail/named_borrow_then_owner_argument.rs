// Expected E0502: explicit &mut activates immediately; not a two-phase reservation.
fn main() {
    let mut values = vec![1, 2];
    let r = &mut values;
    r.push(values.len());
}
