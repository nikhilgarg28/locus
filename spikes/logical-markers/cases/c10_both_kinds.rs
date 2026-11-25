// expect: ok
// One trait for both kinds of type, and `Logical` as an ordinary bound.
extern crate lg;
use lg::*;
fn conv<A, B: From<A>>(a: A) -> B { B::from(a) }
fn only_ghost<G: Logical + Describe>() -> &'static str { G::describe() }
fn main() {
    println!("{}", <u32 as Describe>::describe());
    println!("{}", <Nat as Describe>::describe());
    println!("{}", only_ghost::<Int>());
    let wide: u64 = conv(7u32);      // the standard library's `From`, at runtime types
    let _int: Int = conv(Nat);       // and at logical types
    println!("conv at runtime types gave {}", wide);
}
