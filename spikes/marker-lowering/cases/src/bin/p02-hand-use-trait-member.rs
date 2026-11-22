//! expect: runtime=E0658 check=E0658
//! Plain Rust, written by hand: a probe by name cannot reach a member of a
//! trait. `use Trait::member;` is not stable in rustc 1.97.
use ::locus::{Bool, stub};

trait OrderSpec {
    fn le(_a: Self, _b: Self) -> Bool
    where
        Self: Sized,
    {
        stub()
    }
}

fn main() {
    #[allow(unused_imports)]
    use OrderSpec::le as _;
    println!("expansion: {}", ::locus::__expansion!());
}
