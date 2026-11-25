//! expect: runtime=ok check=ok
//! prints: run() => ([[1, 9], [2]], "locus::Seq<locus::Seq<locus::Nat>>", "locus::Prop")
//! A mutable loan of an element, whose contract gives the contents back
//! from the element's final value; and `nested[0].push(x)` for a
//! `Vec<Vec<u32>>`, through it.
use ::locus::locus;
locus! {
    use ::locus::Vec;
    fn push_into_first(nested: &mut Vec<Vec<u32>>, x: u32, nonempty: @(0 < nested.len()))
        -> @(snap(final!(nested)).elements() === snap(*nested).elements().set(0 as Nat, snap(final!(nested)).elements()[0]))
    {
        let before = logic! { snap(*nested) };
        let (first, restored) = nested.get_mut(0, nonempty);
        let pushed = first.push(x);
        _
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (std::vec::Vec<std::vec::Vec<u32>>, &'static str, &'static str) {
        let mut nested: Vec<Vec<u32>> = Vec::new();
        let _ = nested.push(Vec::from_std(vec![1]));
        let _ = nested.push(Vec::from_std(vec![2]));
        let done = push_into_first(&mut nested, 9, _);
        let model = logic! { nested };
        let shape = logic! { snap(nested).elements()[0].elements().len() == 2 };
        (nested.iter().map(|v| v.to_vec()).collect(), type_of(&model), type_of(&shape))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
