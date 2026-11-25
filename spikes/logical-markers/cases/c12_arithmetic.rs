// expect: ok
// Arithmetic on ghost values can stay Rust code: `Add` on the markers gives the result's type.
extern crate lg;
use lg::*;
fn main() {
    let a: Nat = ghost();
    let b: Nat = ghost();
    let i: Int = ghost();
    let sum = a + b;
    let mixed = i + a;
    println!("a + b : {}", std::any::type_name_of_val(&sum));
    println!("i + a : {}", std::any::type_name_of_val(&mixed));
}
