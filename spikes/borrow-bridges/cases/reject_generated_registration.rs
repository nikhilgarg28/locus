// Limitation of the closed-registry prototype, NOT a proposed Locus rejection.
use borrow_bridge_probe::{BorrowOperation, NativeBorrowBinding, provide_text_borrow};
struct Local(String);
struct GeneratedEvidence;
impl BorrowOperation<str> for Local {
    fn borrow_verified(&self) -> &str {
        &self.0
    }
}
provide_text_borrow!(Local);
impl NativeBorrowBinding<Local, str> for GeneratedEvidence {}
fn main() {}
