use std::borrow::Borrow;
struct Wrapper<T>(T);
impl<T, Q: ?Sized> Borrow<Q> for Wrapper<T>
where
    T: Borrow<Q>,
{
    fn borrow(&self) -> &Q {
        self.0.borrow()
    }
}
fn main() {}
