//! expect: runtime=ok check=ok
//! prints: run() => (4, 3, 12)
//! A closure and a function item, given to a generic function that does
//! not ask for `Model`: nothing is asked of them, and the function's
//! contract speaks of the values around them.
use ::locus::locus;
locus! {
    fn twice(x: u32) -> u32 { x * 2 }
    fn apply<F: Fn(u32) -> u32>(f: F, x: u32) -> (r: u32, @(x == x)) { (f(x), _) }
    fn keep<T: Model, F: FnOnce(T) -> T>(f: F, x: T) -> (r: T, Snap<T>) {
        let before = logic! { snap(x) };
        (f(x), before)
    }
    fn run() -> (u32, u32, u32) {
        let k = 1;
        let (a, _) = apply(twice, 2);
        let (b, _) = apply(|x| x + k, 2);
        let (c, _) = keep(move |x: u32| x * 3 + k - 1, 4);
        (a, b, c)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
