//! expect: runtime=ok check=ok
//! prints: run(&vec![4, 5, 6]) => (true, 1)
//! A reference to a ghost local, returned from a function and used later.
//! (With a conversion at each use this was E0716: the reference outlived
//! the temporary.)
use ::locus::locus;

locus! {
    fn pick<'a>(a: &'a Nat, b: &'a Nat) -> &'a Nat { a }
    fn read(n: &Nat) -> u32 { 1 }

    fn run(v: &Vec<u32>) -> (bool, u32) {
        let before = logic! { v.len() };
        let after = logic! { v.len() + 1 };
        let kept = pick(&before, &after);
        let n = v.len();
        let unchanged = prove!(before == n);
        (std::ptr::eq(kept, &before), read(kept))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run(&vec![4, 5, 6]) => {:?}", run(&vec![4, 5, 6]));
}
