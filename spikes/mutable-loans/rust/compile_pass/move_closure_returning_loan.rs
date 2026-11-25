// A FnOnce helper can return the captured mutable reference when moved in.
fn run<F: FnOnce() -> R, R>(f: F) -> R {
    f()
}
fn lend(value: &mut i32) -> &mut i32 {
    run(move || value)
}
fn main() {
    let mut x = 0;
    *lend(&mut x) = 2;
    assert_eq!(x, 2);
}
