use borrow_bridge_probe::{StdText, native_borrow};
fn escape() -> &'static str {
    let owner = String::from("alice");
    native_borrow::<String, str, StdText>(&owner)
}
fn main() {
    let _ = escape();
}
