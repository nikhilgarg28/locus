//! expect: runtime=ok check=ok
//! prints: build() => 7
//! The first spike's `c01_family`: the associated type is a proof at `Nat`,
//! and a runtime struct stores one. By the draft this program is accepted.
//! In this mode `T::Out` in program text is the marker's, `LogicalData`,
//! and a proof is `Erased`.
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

    fn build() -> u32 {
        let b = keep_out::<Nat>(_);
        let h = Holder::<Nat> { n: 7, out: b };
        h.n
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("build() => {:?}", build());
}
