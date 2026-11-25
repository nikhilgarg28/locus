//! expect: runtime=ok check=ok
//! prints: run() => [1, 2, 3]
//! A ghost snapshot of a `&mut`, then a new mutable use of the place, then
//! the snapshot again. It must compile: `Snap<T>` is one marker for every
//! `T`, so it keeps no borrow. Written three ways: typed by an annotation
//! that names the `&mut` type, typed by the parameter it is passed to, and
//! mentioned by logical code alone.
use ::locus::locus;

locus! {
    fn witness(before: Snap<Vec<u32>>) -> u32 { 0 }
    fn keep<G: Logical>(g: G) -> G { g }

    fn run() -> Vec<u32> {
        let mut v = vec![1u32];
        let r = &mut v;
        let annotated: Snap<&mut Vec<u32>> = logic! { snap(r) };
        let inferred = logic! { snap(*r) };
        let untyped = logic! { snap(*r) };
        r.push(2);
        v.push(3);
        let kept = keep(annotated);
        let n = witness(inferred);
        let grew = prove!(v.len() == untyped.len() + 2);
        v
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
