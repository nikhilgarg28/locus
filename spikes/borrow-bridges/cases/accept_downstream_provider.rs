use borrow_bridge_probe::{BorrowOperation, provide_text_borrow};
use std::borrow::Borrow;
struct Local(String);
impl BorrowOperation<str> for Local {
    fn borrow_verified(&self) -> &str {
        &self.0
    }
}
provide_text_borrow!(Local);
fn main() {
    let value = Local("bob".into());
    assert_eq!(<Local as Borrow<str>>::borrow(&value), "bob");
}
