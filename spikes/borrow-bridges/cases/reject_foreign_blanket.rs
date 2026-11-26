use borrow_bridge_probe::BorrowOperation;
use std::borrow::Borrow;
impl<T: BorrowOperation<str>> Borrow<str> for T {
    fn borrow(&self) -> &str {
        self.borrow_verified()
    }
}
fn main() {}
