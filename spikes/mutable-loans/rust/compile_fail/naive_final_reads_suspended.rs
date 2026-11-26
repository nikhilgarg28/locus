// Expected E0502. final! reads NO place, so a typing skeleton must not implement
// it by borrowing the now-suspended parent reference just to learn its type.
fn final_placeholder<T>(_: &T) {}
fn main() {
    let mut values = [1, 2];
    let r = &mut values;
    let x = &mut r[0];
    final_placeholder(&*r);
    *x = 3;
}
