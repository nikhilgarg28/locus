//! expect: runtime=ok check=ok
//! prints: run() => 2
//! ask: bound Bool: OrderSpec => does not hold
//! ask: bound Int: OrderSpec => holds
//! The division of labour. `Bool` does not implement `OrderSpec`, and
//! program text uses it where the bound is asked: in a call of a runtime
//! function, and in logical code. rustc accepts both, because the bound is
//! met at the marker. The driver answers that the bound does not hold.
use ::locus::locus;

locus! {
    trait OrderSpec {
        logic fn le(a: Self, b: Self) -> Bool;
    }
    impl OrderSpec for Int {
        logic fn le(a: Int, b: Int) -> Bool { a <= b }
    }

    logic fn is_max<T: OrderSpec>(m: T, a: T, b: T) -> Prop { T::le(a, m) && T::le(b, m) }

    fn show<T: OrderSpec>(x: T) -> u32 { 1 }
    fn bounded<T: OrderSpec>(m: T, h: @(is_max(m, m, m))) -> u32 { 1 }

    fn run(flag: Bool) -> u32 {
        let top: @(is_max(flag, flag, flag)) = _;
        show(flag) + bounded(flag, top)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost()));
}
