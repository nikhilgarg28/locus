//! expect: runtime=ok check=ok
//! prints: run() => "locus::Prop"
//! A `Bool` where a parameter of type `Prop` is wanted: a comparison given
//! to a logic function that takes a proposition. Where the source shows a
//! proposition (`@(..)`, `prove!`, `==>`, a function that returns `Prop`)
//! the skeleton asks for a `Prop` or a `Bool`; a parameter's type it does
//! not see.
use ::locus::locus;

locus! {
    logic fn twice(p: Prop) -> Prop { p && p }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(x: u32) -> &'static str {
        let p = prop!(twice(x > 0));
        type_of(&p)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1));
}
