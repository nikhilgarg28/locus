//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Prop", "locus::Erased")
//! `forall` and `exists`, as closures whose parameters the source types.
use ::locus::locus;

locus! {
    logic fn all_small(xs: Seq<Nat>, bound: Nat) -> Prop {
        forall |i: Nat| i < xs.len() ==> xs[i] <= bound
    }
    logic fn cancels(k: Int) -> Prop { exists |j: Int, zero: Int| k + j === zero && zero == 0 }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(x: u32, v: &Vec<u32>) -> (&'static str, &'static str) {
        let p = prop!(forall |n: Int| n + x === x + n && all_small(v, x));
        let all: @(forall |n: Int| n + x === x + n) = logic! { |n: Int| prove!(n + x === x + n) };
        (type_of(&p), type_of(&all))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(1, &vec![1]));
}
