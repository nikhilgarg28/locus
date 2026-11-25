//! expect: runtime=ok check=ok
//! prints: run() => (2, 7)
//! A proposition in a struct field's type has a typing skeleton: the other
//! fields are in scope in it, and a named component of the field's own type.
use ::locus::locus;
locus! {
    logic fn sorted(xs: Seq<Nat>) -> Prop { forall |i: Nat, j: Nat| i < j && j < xs.len() ==> xs[i] <= xs[j] }
    struct Interval<T> {
        lo: u32,
        hi: u32,
        tag: T,
        ordered: @(lo <= hi),
        marks: Vec<(m: u32, @(lo <= m && m <= hi))>,
        steps: Vec<u32>,
        ascending: @(sorted(steps) && steps.len() as Int <= hi - lo),
    }
    fn run() -> (u32, u32) {
        let i = Interval { lo: 2, hi: 7, tag: "x", ordered: _, marks: Vec::new(), steps: vec![], ascending: _ };
        (i.lo, i.hi)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
