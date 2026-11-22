//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Seq<locus::Nat>", "(locus::Nat, locus::traits::Maybe<locus::Int>)", "d6_no_overlap::Point_Snap<locus::Nat>", "locus::Seq<d6_no_overlap::Point_Snap<locus::Nat>>", "locus::traits::Maybe<(locus::Nat, locus::Seq<locus::Bool>)>", "locus::Nat")
//! Required case 6: the family is normalized through references, boxes,
//! arrays, slices, tuples, `Option`, a declared type and a generic one,
//! in concrete and in generic code, by structural implementations alone.
//! There is no blanket implementation for them to overlap.
use ::locus::locus;
locus! {
    struct Point { x: u32 }
    fn snapshot<T: Model + ?Sized>(x: &T) -> Snap<T> { logic! { snap(x) } }
    logic fn same_type<T>(a: T, b: T) -> Prop { a === b }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str, &'static str) {
        let mut arr = Box::new([1u32, 2, 3]);
        let nested = &mut arr;
        let pair = (1u8, Some(-1i16));
        let points = [Point { x: 1 }];
        let slice: &[Point] = &points;
        let deep: Option<(usize, Vec<bool>)> = None;
        let a = logic! { snap(nested) };
        let agree = logic! { same_type(a, snapshot(&nested)) && same_type(snap(pair), snapshot(&pair)) && same_type(snap(slice), snapshot(slice)) };
        (type_of(&a), type_of(&snapshot(&pair)), type_of(&snapshot(&points[0])), type_of(&snapshot(slice)), type_of(&snapshot(&deep)), type_of(&snapshot(&&&7u64)))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
