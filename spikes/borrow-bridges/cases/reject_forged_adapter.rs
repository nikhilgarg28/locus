use borrow_bridge_probe::NativeBorrowBinding;
struct Forged;
impl NativeBorrowBinding<String, str> for Forged {}
fn main() {}
