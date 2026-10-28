// Question 5: inside vec![..], prove! reads a value after it has moved.
fn late() -> usize {
    let v = vec![1, 2, 3];
    let moved = v;
    let proofs: Vec<::__locus::Proof![true]> = vec![prove!(v.len() == 3)]; //~ error: v has moved
    moved.len() + proofs.len()
}
fn main() { println!("late = {}", late()); }
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
// expect: check=E0382 run=ok
