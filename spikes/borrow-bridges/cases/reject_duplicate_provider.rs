use borrow_bridge_probe::{BorrowOperation, provide_text_borrow};
use std::borrow::Borrow;
struct Local(String);
impl BorrowOperation<str> for Local {
    fn borrow_verified(&self) -> &str {
        &self.0
    }
}
provide_text_borrow!(Local);
impl Borrow<str> for Local {
    fn borrow(&self) -> &str {
        &self.0
    }
}
fn main() {}
