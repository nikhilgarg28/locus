//! expect: runtime=ok check=ok
//! prints: run() => (3, 2, 1, "locus::VecSnap<locus::Nat>", "locus::Seq<locus::Nat>", "locus::Seq<locus::Nat>", [5, 6])
//! Locus's `Vec`: a struct around the standard vector with a ghost field
//! for the elements' snapshots and NO bound on `T`. It is named, built and
//! read through `Deref` at an element type with no model (a closure). Its
//! snapshot at `&mut u32` keeps no borrow. `into_std` and `from_std` are
//! the way out and back.
use ::locus::locus;
locus! {
    use ::locus::Vec;
    fn count<T>(v: &Vec<T>) -> usize { v.iter().count() }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    logic fn takes_seq(xs: Seq<Nat>) -> Nat { xs.len() }
    fn run() -> (u32, usize, usize, &'static str, &'static str, &'static str, std::vec::Vec<u32>) {
        let k = 1u32;
        let closures = Vec::from_std(vec![move |x: u32| x + k]);
        let (mut a, mut b) = (1u32, 2u32);
        let mut borrowed: Vec<&mut u32> = Vec::new();
        let grew = borrowed.push(&mut a);
        let again = borrowed.push(&mut b);
        let held = logic! { snap(borrowed) };
        let model = logic! { borrowed };
        let n = count(&borrowed);
        drop(borrowed);
        a = 3;
        let still = logic! { takes_seq(held.elements()) + takes_seq(model) };
        let std_one = Vec::from_std(vec![5u32, 6]).into_std();
        (a, n, count(&closures), type_of(&held), type_of(&logic! { held.elements() }), type_of(&model), std_one)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
