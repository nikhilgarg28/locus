//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => (3, 1, 1, 3)
//! Ghost locals bound in patterns.
use ::locus::locus;

locus! {
    fn takes_nat(n: Nat) -> u32 { 1 }
    fn measure(v: &Vec<u32>) -> (n: usize, @(n == v.len())) { (v.len(), _) }
    fn remember(v: &Vec<u32>) -> Option<Nat> { Some(logic! { v.len() }) }

    fn run(v: &Vec<u32>) -> (usize, u32, u32, usize) {
        // From a call: the callee's signature gives the type.
        let (n, exact) = measure(v);
        // A tuple pattern over a tuple that holds a `logic!`: `before` is
        // mentioned only by logical code, and `passed` by runtime code.
        let (count, before, passed) = (v.len(), logic! { v.len() }, logic! { v.len() });
        let same = prove!(before == count);
        // Bound by `if let`: the scrutinee's type gives it.
        let from_option = if let Some(len) = remember(v) { takes_nat(len) } else { 0 };
        (n, takes_nat(passed), from_option, count)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
