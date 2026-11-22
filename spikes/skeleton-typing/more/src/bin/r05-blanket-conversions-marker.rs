//! expect: runtime=E0119 check=E0119
//! r04 with the implementation for the marker written regardless
//! (`always_marker_impl`): does it collide with the blanket conversions?
use ::locus::locus;

locus! {
    #![spike(always_marker_impl)]

    trait From2<T: Logical>: Logical {
        logic fn from2(t: T) -> Self;
    }
    trait Widen<U: Logical>: Logical {
        logic fn widen(self) -> U;
    }

    impl<T: Logical> From2<T> for T {
        logic fn from2(t: T) -> T { t }
    }
    impl<T: Logical, U: From2<T>> Widen<U> for T {
        logic fn widen(self) -> U { U::from2(self) }
    }
    impl From2<Nat> for Int {
        logic fn from2(t: Nat) -> Int { t }
    }

    fn to_int<T: Widen<Int>>(t: T) -> u32 { 1 }
    fn to_nat<T: Widen<Nat>>(t: T) -> u32 { 2 }

    fn run(n: Nat, i: Int) -> u32 {
        let wide: Int = logic! { n.widen() };
        to_int(n) + to_int(i) + to_int(wide) + to_nat(n) + to_nat(n)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
