//! expect: runtime=reject check=reject
//! A reviewer's counterexample: a mangled name is writable. An ordinary
//! method named `__logic_answer` in a trait that also has the logic function
//! `answer`, whose stub has that name. Here the rejection of reserved names
//! is turned off, to reproduce it.
use ::locus::locus;

locus! {
    #![spike(allow_reserved_names)]

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
