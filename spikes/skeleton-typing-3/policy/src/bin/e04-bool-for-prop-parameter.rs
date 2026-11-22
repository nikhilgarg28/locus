//! expect: runtime=reject check=reject
//! A deliberate type error in logical code: a `Bool` where a parameter of
//! type `Prop` is wanted.
use ::locus::locus;
locus! {
    logic fn twice(p: Prop) -> Prop { p && p }
    fn run(flag: bool) -> u32 {
        let from_a_place = logic! { twice(flag) };
        let from_a_value = logic! { twice(true) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(true));
}
