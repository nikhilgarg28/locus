//! expect: runtime=E0119 check=E0119
//! prints: run() => (1, 2)
//! A reviewer's counterexample. A trait with a method that runs, implemented
//! for two runtime types that differ only in a logical argument. With a type
//! for each logical type these are two implementations, and the program
//! prints which logical type it was given: something that runs depends on
//! an erased type. With one marker in program text they are one type.
use ::locus::locus;

locus! {
    trait Tag {
        fn tag(&self) -> u8;
    }
    impl Tag for Vec<Int> {
        fn tag(&self) -> u8 { 1 }
    }
    impl Tag for Vec<Nat> {
        fn tag(&self) -> u8 { 2 }
    }

    fn run() -> (u8, u8) {
        let ints: Vec<Int> = Vec::new();
        let nats: Vec<Nat> = Vec::new();
        (ints.tag(), nats.tag())
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
