//! expect: runtime=ok check=ok
//! prints: build() => (15, 4, 4)
//! An associated logical type that differs between two logical types: `Int`
//! at `Int`, and a proof at `Nat`. It is used in a signature and stored in a
//! runtime struct. (The first spike's `c01_family.rs`, through a lowering.)
use ::locus::locus;

locus! {
    trait Family: Logical {
        type Out: Logical;
    }
    impl Family for Int {
        type Out = Int;
    }
    impl Family for Nat {
        type Out = @(1 + 1 == 2);
    }

    fn keep_out<T: Family>(x: T::Out) -> T::Out { x }

    struct Holder<T: Family> {
        n: u32,
        out: T::Out,
    }

    fn build() -> (u32, usize, usize) {
        let a: Int = keep_out::<Int>(logic! { 5 });
        let b = keep_out::<Nat>(_);
        let h = Holder::<Nat> { n: 7, out: b };
        let i = Holder::<Int> { n: 8, out: logic! { 3 } };
        let stored = i.out;
        let again: Int = keep_out::<Int>(stored);
        (h.n + i.n, std::mem::size_of::<Holder<Nat>>(), std::mem::size_of::<Holder<Int>>())
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("build() => {:?}", build());
}
