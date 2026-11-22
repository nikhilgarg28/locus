//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Erased", "locus::Nat", "locus::Erased")
//! Holes, lemma calls and proof terms are all `Erased` to rustc. A hole
//! that stands for a value has the type of the parameter it fills.
use ::locus::locus;

locus! {
    logic fn add_zero(n: Int) -> @(n + 0 === n) { _ }
    logic fn pick(n: Nat, same: @(n === n)) -> Nat { n }
    prop Even(n: Nat) {
        Zero => { n == 0 }
        Step(m: Nat) => { n == m + 2 && Even(m) }
    }
    logic fn half(n: Nat, even: @Even(n)) -> Nat {
        match even {
            Even::Zero @ is_zero => 0,
            Even::Step(m) @ And::Intro(is_step, smaller) => 1 + half(m, smaller),
        }
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run() -> (&'static str, &'static str, &'static str) {
        let lemma = logic! { add_zero(3) };
        let n = logic! { pick(_, _) };
        let four = logic! { Even::Step(2) };
        (type_of(&lemma), type_of(&n), type_of(&four))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
