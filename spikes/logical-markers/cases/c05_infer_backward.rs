// expect: ok
// A ghost local with no annotation, used later where a type is written: rustc infers it backward.
extern crate lg;
use lg::*;
fn main() {
    let g = ghost();                 // let g = logic! { .. };
    println!("rustc inferred {}", std::any::type_name_of_val(&g));
    takes_nat(g);
}
