use borrow_bridge_probe::{Identity, StdText, native_borrow};
fn main() {
    let key = String::from("alice");
    assert_eq!(native_borrow::<String, str, StdText>(&key), "alice");
    assert_eq!(native_borrow::<str, str, Identity>("alice"), "alice");
}
