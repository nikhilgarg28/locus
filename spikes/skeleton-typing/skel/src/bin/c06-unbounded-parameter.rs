//! expect: runtime=ok check=ok
//! prints: run() => (1, 1)
//! The snapshot of a value whose type is a type parameter. By RFC 0002 it
//! is `Snap<T>`, abstract, for every `T`. To rustc `T` needs a bound, in
//! `unbounded` and in the signature of `takes`; `bounded` writes it.
use ::locus::locus;

locus! {
    fn unbounded<T>(x: &T) -> u32 {
        let before = logic! { snap(*x) };
        let same: @(snap(*x) === before) = _;
        1
    }
    fn takes<T>(s: Snap<T>) -> u32 { 1 }
    fn bounded<T: Snapshot>(x: &T) -> u32 {
        let before = logic! { snap(*x) };
        let same: @(snap(*x) === before) = _;
        takes::<T>(before)
    }

    fn run() -> (u32, u32) { (unbounded(&1u8), bounded(&1u8)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
