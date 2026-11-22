//! expect: runtime=error check=error
//! r07 as `locus!` now takes it: an identifier of the source that starts
//! with `__logic_` or `__locus_` is rejected.
use ::locus::locus;

locus! {
    trait Oracle {
        logic fn answer() -> Nat;
        fn __logic_answer() -> u32 { 42 }
    }

    struct Delphi;
    impl Oracle for Delphi {
        logic fn answer() -> Nat { 42 }
    }

    fn run() -> u32 { Delphi::__logic_answer() }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
