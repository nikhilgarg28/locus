//! expect: runtime=ok check=ok
//! prints: run() => 4
//! Plain Rust, written by hand: r02 lowered another way, which this spike's
//! lowering does not do. The trait's argument is a stub in the header of
//! each `impl` and in each bound, where it is the type of no value. Both
//! implementations then exist, and rustc itself tells `Spec<Int>` from
//! `Spec<Nat>`. The marker has the trait at every argument.
use ::locus::{__locus_Int, __locus_Nat, LogicalData, stub};

trait Spec<A> {
    fn __logic_spec(_x: Self) -> A
    where
        Self: Sized,
    {
        stub()
    }
}
impl<A> Spec<A> for LogicalData {}
impl Spec<__locus_Int> for u32 {}
impl Spec<__locus_Nat> for u32 {}

fn as_int<T: Spec<__locus_Int>>(_x: T) -> u32 {
    1
}
fn as_nat<T: Spec<__locus_Nat>>(_x: T) -> u32 {
    2
}

fn run() -> u32 {
    // A runtime type at each bound, and a logical value at one.
    as_int(7u32) + as_nat(7u32) + as_int(LogicalData)
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
