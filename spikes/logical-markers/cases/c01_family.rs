// expect: ok
// The reviewer's example: an associated type that erases differently at two logical types,
// used in a signature and stored in a runtime struct.
extern crate lg;
use lg::*;

trait Family: Logical { type Out: Logical; }
impl Family for Int { type Out = Int; }
impl Family for Nat { type Out = Erased; }

fn keep_out<T: Family>(x: T::Out) -> T::Out { x }

struct Holder<T: Family> { n: u32, out: T::Out }

fn main() {
    let _a: Int = keep_out::<Int>(Int);
    let _b: Erased = keep_out::<Nat>(Erased);
    let h = Holder::<Nat> { n: 7, out: Erased };
    println!("Holder<Nat>.n = {}, size = {} bytes", h.n, std::mem::size_of::<Holder<Nat>>());
    println!("size of (u32, Nat, Seq<Int>) = {} bytes", std::mem::size_of::<(u32, Nat, Seq<Int>)>());
}
