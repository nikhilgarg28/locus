//! expect: runtime=reject check=reject
//! prints: run() => ((8, 1), (16, 2))
//! A reviewer's counterexample: content that runs and is not a method. An
//! ordinary associated type and an associated constant, different at `Int`
//! and at `Nat`, read by generic code that runs.
use ::locus::locus;

locus! {
    trait Description {
        type Output;
        const WIDTH: usize;
    }
    impl Description for Int {
        type Output = u8;
        const WIDTH: usize = 8;
    }
    impl Description for Nat {
        type Output = u16;
        const WIDTH: usize = 16;
    }

    fn describe<T: Description>(x: T) -> (usize, usize) {
        (T::WIDTH, std::mem::size_of::<T::Output>())
    }

    fn run(i: Int, n: Nat) -> ((usize, usize), (usize, usize)) {
        (describe(i), describe(n))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(::locus::ghost(), ::locus::ghost()));
}
