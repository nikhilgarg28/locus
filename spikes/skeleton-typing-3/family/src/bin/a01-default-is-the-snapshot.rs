//! expect: runtime=ok check=ok
//! prints: run() => ("a01_default_is_the_snapshot::Point_Snap<locus::Nat, locus::Int>", "locus::Nat", "locus::Nat", "locus::Prop")
//! The identity default: a struct of plain data, captured and then
//! projected, against projected and then captured, and an integer field
//! compared with an old value.
use ::locus::locus;
locus! {
    struct Point { x: u32, y: i32 }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn bump(p: &mut Point) -> @(final!(p).x == p.x + 1) {
        let before = logic! { *p };
        p.x += 1;
        let grew: @(p.x === before.x + 1) = _;
        _
    }
    fn run() -> (&'static str, &'static str, &'static str, &'static str) {
        let mut p = Point { x: 1, y: -1 };
        let before = logic! { p };
        let then_projected = logic! { before.x };
        let projected_then = logic! { p.x };
        let same = logic! { before.x === p.x };
        let _ = bump(&mut p);
        (type_of(&before), type_of(&then_projected), type_of(&projected_then), type_of(&same))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
