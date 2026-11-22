//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Seq<locus::BoundedU32>", 1)
//! A generic function of one `locus!` invocation passes its parameter on
//! to a function of another invocation, and holds a type of another
//! invocation whose field is a snapshot. Lowering sees neither need.
use ::locus::locus;
mod library {
    use ::locus::locus;
    locus! {
        pub fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
        pub fn kept<T>(x: Vec<T>) -> &'static str { type_of(&logic! { snap(x) }) }
        pub struct Keep<T> { pub now: T, pub before: Snap<T> }
    }
}
locus! {
    fn passes_on<T>(x: Vec<T>) -> &'static str { library::kept(x) }
    fn holds<T>(k: &library::Keep<T>) -> u32 { 1 }
    fn run() -> (&'static str, u32) {
        (passes_on(vec![1u32]), holds(&library::Keep { now: 1u8, before: logic! { snap(1u8) } }))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
