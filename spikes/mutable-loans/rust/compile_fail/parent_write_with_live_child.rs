// Expected E0506: the parent is suspended while the child is still used.
fn main() {
    let mut value = 0;
    let outer = &mut value;
    let inner = &mut *outer;
    *outer = 2;
    *inner = 3;
}
