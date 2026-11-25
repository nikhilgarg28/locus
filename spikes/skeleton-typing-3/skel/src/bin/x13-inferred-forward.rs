//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Nat", "locus::Erased", "locus::Seq<locus::Nat>")
//! Ghost locals that the first mode could not type without help: one that
//! is only copied, one in a format string, one passed only to a generic
//! function. Each takes its type from its logical code, forward.
use ::locus::locus;

locus! {
    logic fn add_zero(n: Int) -> @(n + 0 === n) { _ }
    fn keep<G: Logical>(g: G) -> G { g }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run(v: &Vec<u32>) -> (&'static str, &'static str, &'static str) {
        let before = logic! { v.len() };
        let again = before;
        let lemma = logic! { add_zero(3) };
        let kept = keep(lemma);
        let whole = logic! { *v };
        let shown = format!("{whole:?}");
        (type_of(&again), type_of(&kept), type_of(&whole))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&vec![1]));
}
