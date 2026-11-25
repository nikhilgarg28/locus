//! expect: runtime=reject check=reject
//! Where a `Bool` still has to be written as a proposition under the
//! default: given to a parameter of type `Prop`, bound to a ghost local
//! annotated `Prop`, and in a generic position.
use ::locus::locus;
locus! {
    logic fn twice(p: Prop) -> Prop { p && p }
    logic fn same<T: Logical>(t: T) -> T { t }
    fn run() -> u32 {
        let flag = true;
        let to_a_parameter = logic! { twice(flag) };
        let annotated: Prop = logic! { flag };
        let generic: Prop = logic! { same(model(flag)) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
