//! expect: runtime=ok check=ok
//! prints: build() => (15, 4, 4, "locus::LogicalData")
//! ask: assoc Int: Family::Out => Int
//! ask: assoc Nat: Family::Out => Seq<Nat>
//! ask: assoc Seq<Int>: Family::Out => Seq<Int>
//! ask: bound Bool: Family => does not hold
//! ask: in keep_out: assoc T: Family::Out => <T as Family>::Out
//! An associated logical type that differs between logical types. The
//! driver normalizes it on stubs. In program text `T::Out` at the marker is
//! the marker, in a signature and stored in a runtime struct.
use ::locus::locus;

locus! {
    trait Family: Logical {
        type Out: Logical;
    }
    impl Family for Int {
        type Out = Int;
    }
    impl Family for Nat {
        type Out = Seq<Nat>;
    }
    impl<T: Family> Family for Seq<T> {
        type Out = Seq<T::Out>;
    }

    fn keep_out<T: Family>(x: T::Out) -> T::Out { x }

    struct Holder<T: Family> {
        n: u32,
        out: T::Out,
    }

    fn build() -> (u32, usize, usize, &'static str) {
        let a: Int = keep_out::<Int>(logic! { 5 });
        let b: Seq<Nat> = keep_out::<Nat>(logic! { Seq::empty() });
        let h = Holder::<Nat> { n: 7, out: b };
        let i = Holder::<Int> { n: 8, out: a };
        (
            h.n + i.n,
            std::mem::size_of::<Holder<Nat>>(),
            std::mem::size_of::<Holder<Int>>(),
            std::any::type_name::<<Nat as Family>::Out>(),
        )
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("build() => {:?}", build());
}
