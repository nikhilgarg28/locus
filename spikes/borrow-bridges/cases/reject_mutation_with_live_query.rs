use borrow_bridge_probe::{StdText, native_borrow};
fn main() {
    let mut owner = String::from("alice");
    let query = native_borrow::<String, str, StdText>(&owner);
    owner.clear();
    println!("{query}");
}
