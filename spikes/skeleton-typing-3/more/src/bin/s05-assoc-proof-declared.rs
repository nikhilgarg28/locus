//! expect: runtime=ok check=ok
//! ask: assoc Nat: Family::Out => Erased
//! ask: assoc Bool: Family::Out => (Nat, Int)
//! ask: assoc Int: Family::Out => Int
//! An associated type given as a proof type, and one given as a tuple. As
//! declarations they compile, and the driver normalizes each on stubs. The
//! empty implementation for the marker can only say `LogicalData`: s06 and
//! s07 use these types in program text.
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
    impl Family for Bool {
        type Out = (Nat, Int);
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
