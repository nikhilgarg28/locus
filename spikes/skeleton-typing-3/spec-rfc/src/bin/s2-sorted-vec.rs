//! expect: runtime=ok check=ok
//! prints: run() => [1, 3, 4, 7, 9]
//! Specification 2: a sorted vector. A quantified `sorted`, a recursive
//! count, a lemma about each, and an `insert` whose contract says that the
//! result is sorted and one longer.
use ::locus::locus;

locus! {
    /// `xs` is in ascending order.
    logic fn sorted(xs: Seq<Nat>) -> Prop {
        forall |i: Nat, j: Nat| i < j && j < xs.len() ==> xs[i] <= xs[j]
    }

    /// How many elements of `xs` are less than `x`.
    logic fn below(xs: Seq<Nat>, x: Nat) -> Nat {
        if xs.len() == 0 {
            0
        } else {
            (if xs[0] < x { 1 as Nat } else { 0 as Nat }) + below(xs.skip(1), x)
        }
    }

    logic fn below_is_bounded(xs: Seq<Nat>, x: Nat) -> @(below(xs, x) <= xs.len()) { _ }

    /// In a sorted sequence, the elements below `x` are exactly a prefix.
    logic fn below_is_a_prefix(xs: Seq<Nat>, x: Nat, ordered: @sorted(xs))
        -> @(forall |k: Nat| k < xs.len() ==> (k < below(xs, x) <==> xs[k] < x)) { _ }

    /// Inserting between what is not above and what is not below keeps order.
    logic fn insert_keeps_order(
        xs: Seq<Nat>, at: Nat, x: Nat, ordered: @sorted(xs),
        before: @(forall |k: Nat| k < at ==> xs[k] <= x),
        after: @(forall |k: Nat| at <= k && k < xs.len() ==> x <= xs[k]),
    ) -> @(sorted(xs.insert(at, x))) { _ }

    pub struct SortedVec {
        items: Vec<u32>,
        ordered: @(sorted(items)),
    }

    impl Model for SortedVec {
        type Logic = Seq<Nat>;
        logic fn model(v: &SortedVec) -> Seq<Nat> { v.items }
    }

    impl SortedVec {
        pub fn new() -> (v: SortedVec, @(v.len() == 0)) {
            (SortedVec { items: Vec::new(), ordered: _ }, _)
        }

        pub fn insert(&mut self, x: u32)
            -> (at: usize, @(sorted(final!(self)) && final!(self).len() == self.len() + 1),
                @(at == below(*self, x) && final!(self)[at] == x))
        {
            let old = logic! { model(self) };
            let mut at = 0;
            let mut before: @(forall |k: Nat| k < at ==> self.items[k] < x) = _;
            let mut in_range: @(at <= self.len()) = _;
            while at < self.items.len() && self.items[at] < x {
                at += 1;
                before = _;
                in_range = _;
            }
            let after: @(forall |k: Nat| at <= k && k < self.len() ==> x <= self.items[k]) = _;
            let prefix = logic! { below_is_a_prefix(old, x, _) };
            let counted: @(at == below(old, x)) = _;
            self.items.insert(at, x);
            let kept = logic! { insert_keeps_order(old, at, x, _, _, after) };
            (at, kept, _)
        }

        pub fn as_slice(&self) -> (s: &[u32], @(s === *self && sorted(s))) {
            (&self.items, _)
        }
    }

    fn run() -> Vec<u32> {
        let (mut v, _) = SortedVec::new();
        for x in [4, 9, 1, 7, 3] {
            let _ = v.insert(x);
        }
        let (s, _) = v.as_slice();
        s.to_vec()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
