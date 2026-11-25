//! expect: runtime=ok check=ok
//! prints: run() => "locus::Prop"
//! The same with `=== true` written.
use ::locus::locus;
locus! {
    logic fn twice(p: Prop) -> Prop { p && p }
    logic fn same<T: Logical>(t: T) -> T { t }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> &'static str {
        let flag = true;
        let to_a_parameter = logic! { twice(flag === true) };
        let annotated: Prop = logic! { flag === true };
        let generic: Prop = logic! { same(flag === true) };
        type_of(&to_a_parameter)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
