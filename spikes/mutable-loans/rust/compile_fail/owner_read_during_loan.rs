// Expected E0502: observing the whole owner while the returned loan is live.
fn main() {
    let mut values = vec![1, 2];
    let x = &mut values[0];
    println!("{:?}", values);
    *x = 3;
}
