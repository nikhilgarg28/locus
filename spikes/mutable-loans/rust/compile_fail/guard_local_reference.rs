// Expected E0713: the local guard's destructor needs the field being returned.
struct Guard(i32);
impl Drop for Guard {
    fn drop(&mut self) {}
}
fn bad<'a>() -> &'a mut i32 {
    let mut guard = Guard(0);
    &mut guard.0
}
fn main() {}
