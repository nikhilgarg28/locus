//! expect: runtime=ok check=ok
//! prints: run() => ((true, 2), (false, 0))
//! Specification 3: binary search over a slice. The contract says that the
//! found flag is true exactly when the value is there, and that an index
//! returned with it holds an equal element. The loop keeps its proofs in
//! variables that are proved again after each assignment.
use ::locus::locus;

locus! {
    logic fn sorted(xs: Seq<Int>) -> Prop {
        forall |i: Nat, j: Nat| i <= j && j < xs.len() ==> xs[i] <= xs[j]
    }

    /// `x` is somewhere in `xs`.
    logic fn contains(xs: Seq<Int>, x: Int) -> Prop {
        exists |i: Nat| i < xs.len() && xs[i] == x
    }

    /// Everything up to an element below `x`, in a sorted sequence, is below `x`.
    logic fn below_up_to(xs: Seq<Int>, x: Int, m: Nat, ordered: @sorted(xs), less: @(m < xs.len() && xs[m] < x))
        -> @(forall |k: Nat| k <= m ==> xs[k] < x) { _ }

    logic fn above_from(xs: Seq<Int>, x: Int, m: Nat, ordered: @sorted(xs), more: @(m < xs.len() && x < xs[m]))
        -> @(forall |k: Nat| m <= k && k < xs.len() ==> x < xs[k]) { _ }

    /// The midpoint does not leave the range, and the range shrinks.
    logic fn midpoint(lo: Nat, hi: Nat, open: @(lo < hi))
        -> @(lo as Int + (hi - lo) / 2 >= lo as Int && lo as Int + (hi - lo) / 2 < hi as Int) { _ }

    pub fn search(xs: &[i32], x: i32, ordered: @sorted(xs))
        -> (found: bool, at: usize,
            @(found === true <==> contains(xs, x)),
            @(found === true ==> at < xs.len() && xs[at] == x))
    {
        let mut lo = 0;
        let mut hi = xs.len();
        let mut below: @(forall |k: Nat| k < lo ==> xs[k] < x) = _;
        let mut above: @(forall |k: Nat| hi <= k && k < xs.len() ==> x < xs[k]) = _;
        let mut range: @(lo <= hi && hi <= xs.len()) = _;
        while lo < hi {
            let gap = logic! { hi - lo };
            let mid = lo + (hi - lo) / 2;
            let middle: @(mid as Int == lo as Int + (hi - lo) / 2) = _;
            let inside = logic! { midpoint(lo, hi, _) };
            if xs[mid] == x {
                let here: @(mid < xs.len() && xs[mid] == x) = _;
                return (true, mid, _, _);
            } else if xs[mid] < x {
                below = logic! { below_up_to(xs, x, mid, ordered, _) };
                lo = mid + 1;
                below = _;
            } else {
                above = logic! { above_from(xs, x, mid, ordered, _) };
                hi = mid;
            }
            range = _;
            let shrinks: @(hi - lo < gap) = _;
        }
        let empty: @(lo == hi) = _;
        let absent: @(!contains(xs, x)) = _;
        (false, 0, _, _)
    }

    /// A wrapper whose result is only the flag.
    pub fn has(xs: &[i32], x: i32, ordered: @sorted(xs)) -> (r: bool, @(r === true <==> contains(xs, x))) {
        let (found, _, exactly, _) = search(xs, x, ordered);
        (found, exactly)
    }

    fn run() -> ((bool, usize), (bool, usize)) {
        let xs = [1, 3, 5, 8, 13];
        let (f1, i1, _, _) = search(&xs, 5, _);
        let (f2, _) = has(&xs, 6, _);
        ((f1, i1), (f2, 0))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
