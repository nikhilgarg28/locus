//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Prop", "locus::Prop", "locus::Bool")
//! `&&` and `||` with one `Bool` operand and one `Prop` operand, and
//! `!` on a `Bool` where a proposition is wanted. Accepted by default;
//! under the strict variant each is an error that says to write
//! `b === true`.
use ::locus::locus;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let flag = true;
        let (a, b) = (1u32, 2u32);
        let and = logic! { flag && a < b };
        let or = logic! { a < b || flag };
        let not = logic! { !flag };
        let wanted: @(!flag ==> a < b) = _;
        (type_of(&and), type_of(&or), type_of(&not))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
