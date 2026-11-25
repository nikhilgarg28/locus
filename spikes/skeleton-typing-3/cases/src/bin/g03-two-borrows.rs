//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => (true, true)
//! A ghost local borrowed twice, passed to a function that compares the two
//! addresses. They are equal: the local is one place. (With a conversion at
//! each use, the earlier form of the rule, each borrow was of a temporary.)
use ::locus::locus;

locus! {
    fn same(a: &Nat, b: &Nat) -> bool { std::ptr::eq(a, b) }

    fn run(v: &Vec<u32>) -> (bool, bool) {
        let before = logic! { v.len() };
        let first = &before;
        let second = &before;
        (same(&before, &before), same(first, second))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
