//! expect: runtime=ok check=ok
//! prints: run() => "the branch was not taken"
//! r10 with contracts. The inherent method returns `true` with a proof of
//! `b`, and the trait's returns `false` with a proof of `!b`. The caller
//! holds, inside `if b`, what Locus checked as a proof of `!b`: a proof of
//! `False`, if the method that runs is not the one Locus checked.
use ::locus::locus;

locus! {
    use std::marker::PhantomData;

    struct Wrap<T>(PhantomData<T>);

    impl Wrap<Int> {
        fn flag(&self) -> (b: bool, @(b)) { (true, _) }
    }
    trait Flag {
        fn flag(&self) -> (b: bool, @(!b));
    }
    impl<T> Flag for Wrap<T> {
        fn flag(&self) -> (b: bool, @(!b)) { (false, _) }
    }

    fn absurd(b: bool, yes: @(b), no: @(!b)) -> &'static str {
        "the branch that holds a proof of False was taken"
    }

    fn of_nat(x: Wrap<Nat>) -> &'static str {
        let (b, not_b) = x.flag();
        if b {
            absurd(b, _, not_b)
        } else {
            "the branch was not taken"
        }
    }

    fn run() -> &'static str { of_nat(Wrap(PhantomData)) }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
