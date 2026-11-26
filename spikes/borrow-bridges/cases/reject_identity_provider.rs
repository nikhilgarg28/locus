use std::borrow::Borrow;
struct Key;
impl Borrow<Key> for Key {
    fn borrow(&self) -> &Key {
        self
    }
}
fn main() {}
