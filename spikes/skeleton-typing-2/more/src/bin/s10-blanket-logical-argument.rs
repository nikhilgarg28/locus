//! expect: runtime=ok check=ok
//! prints: run() => 1
//! ask: bound Int: Tagged<Nat> => holds
//! ask: bound Int: Tagged<Bool> => does not hold
//! Where s09's arrangement stops. The implementation for every logical type
//! names a logical type as the trait's argument. In its header that is a
//! stub, so that `Tagged<Nat>` is not `Tagged<Bool>`, and in program text
//! the bound `T: Tagged<Nat>` asks for `Tagged<LogicalData>`.
use ::locus::locus;

locus! {
    trait Tagged<K: Logical> {
        logic fn tag() -> K;
    }
    impl<T: Logical> Tagged<Nat> for T {
        logic fn tag() -> Nat { 0 }
    }

    fn tagged<T: Tagged<Nat>>(x: T) -> u32 { 1 }
    fn run(i: Int) -> u32 { tagged(i) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
