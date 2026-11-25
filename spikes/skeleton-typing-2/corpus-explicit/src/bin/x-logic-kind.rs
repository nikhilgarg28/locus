//! Spike case for question 2, not a litmus example: which marker a `logic!`
//! gives depends on the type of its logical code. A lemma (RFC 0002's
//! `logic fn add_zero(n: Int) -> @(n + 0 === n)`) and a logical closure that
//! proves a `forall` both give proofs; the toy, which reads only an
//! annotation, gives `LogicalData` for the two `let`s that have none.
use ::locus::locus;

locus! {
    logic fn add_zero(n: Int) -> @(n + 0 === n) {
        _
    }

    fn needs_lemma(h: @(3 + 0 === 3)) -> u32 {
        1
    }

    fn needs_forall(h: @(forall |n: Int| n === n)) -> u32 {
        1
    }

    fn annotated() -> u32 {
        let lemma: @(3 + 0 === 3) = logic! { add_zero(3) };
        let all: @(forall |n: Int| n === n) = logic! { |n: Int| prove!(n === n) };
        needs_lemma(lemma) + needs_forall(all)
    }

    fn unannotated() -> u32 {
        let lemma = logic! { add_zero(3) };
        let all = logic! { |n: Int| prove!(n === n) };
        needs_lemma(lemma) + needs_forall(all)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("annotated() => {:?}", annotated());
    println!("unannotated() => {:?}", unannotated());
}
