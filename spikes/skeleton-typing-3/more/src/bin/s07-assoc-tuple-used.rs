//! expect: runtime=ok check=ok
//! prints: build() => 7
//! The same with a tuple: the associated type is `(Nat, Int)` at `Bool`. A
//! logical tuple keeps its shape in program text, `(LogicalData,
//! LogicalData)`, and `T::Out` at the marker is `LogicalData`.
use ::locus::locus;

locus! {
    trait Family: Logical {
        type Out: Logical;
    }
    impl Family for Bool {
        type Out = (Nat, Int);
    }

    fn keep_out<T: Family>(x: T::Out) -> T::Out { x }

    fn build(n: Nat, i: Int) -> u32 {
        let pair: (Nat, Int) = (n, i);
        let kept = keep_out::<Bool>(pair);
        7
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("build() => {:?}", build(::locus::ghost(), ::locus::ghost()));
}
