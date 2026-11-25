//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Nat")
//! A runtime place given to a type parameter of a logic function. By RFC
//! 0002 an argument of a logical parameter is the place's model, so `T` is
//! `Nat`. To rustc the place itself, a `u32`, fits as well.
use ::locus::locus;

locus! {
    logic fn same<T>(t: T) -> T { t }
    logic fn twice<T: AddLogic<Output = T>>(x: T) -> T { x + x }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(n: u32) -> (&'static str, &'static str) {
        let a = logic! { same(n) };
        let b = logic! { twice(n) };
        (type_of(&a), type_of(&b))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1));
}
