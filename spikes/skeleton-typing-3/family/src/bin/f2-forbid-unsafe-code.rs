//! expect: runtime=ok check=ok
//! prints: run() => "f2_forbid_unsafe_code::Point_Snap<locus::Nat>"
//! A lowered crate that forbids `unsafe` code: lowering's own `unsafe impl`
//! is not the program's, and rustc's lint does not report it.
#![forbid(unsafe_code)]
use ::locus::locus;
locus! {
    struct Point { x: u32 }
    fn run() -> &'static str {
        let p = Point { x: 1 };
        std::any::type_name_of_val(&logic! { p })
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
