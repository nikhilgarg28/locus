//! expect: runtime=E0277 check=E0277
//! A closure, and a function item, given to a generic function that asks
//! for `Model`: neither has one, and neither can be given one.
use ::locus::locus;
locus! {
    fn twice(x: u32) -> u32 { x * 2 }
    fn apply_read<F: Fn(u32) -> u32 + Model>(f: F, x: u32) -> u32 { let s = logic! { snap(f) }; f(x) }
    fn run() -> (u32, u32) {
        let k = 1;
        (apply_read(|x| x + k, 2), apply_read(twice, 2))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
