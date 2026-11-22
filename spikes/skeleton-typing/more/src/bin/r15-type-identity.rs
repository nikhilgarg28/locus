//! expect: runtime=ok check=ok
//! prints: run() => (true, true)
//! Not a different callee, and still something that runs depending on a
//! logical type: the identity of a type argument. With a type for each
//! logical type, `TypeId` tells `Int` from `Nat`. With one marker it
//! cannot. A listing of resolved calls shows the same item at each call.
use ::locus::locus;

locus! {
    use std::any::TypeId;

    fn same_as_int<T: 'static>(x: T) -> bool {
        TypeId::of::<T>() == TypeId::of::<Int>()
    }

    fn run(i: Int, n: Nat) -> (bool, bool) { (same_as_int(i), same_as_int(n)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
