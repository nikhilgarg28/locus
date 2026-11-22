//! expect: runtime=E0277 check=E0277
//! Required case 7, the other side. Round 2 read a `u32` through an
//! unbounded parameter as another type (`Opaque`). Now it is not read at all.
use ::locus::locus;
locus! {
    fn unbounded<T>(x: &T) -> &'static str { std::any::type_name_of_val(&logic! { snap(x) }) }
    fn run() -> &'static str { unbounded(&1u32) }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
