//! expect: runtime=ok check=ok
//! prints: run() => (6, 4)
//! Contracts on trait methods that have no body: a runtime method with a
//! proof parameter and a proposition on its result, and a lemma declared
//! in the trait. Their propositions are typed in a hidden provided method.
use ::locus::locus;
locus! {
    trait Shape {
        logic fn area_spec(self: &Self) -> Nat;
        logic fn area_is_bounded(s: &Self, limit: Nat, h: @(Self::area_spec(s) <= limit)) -> @(Self::area_spec(s) < limit + 1);
        fn area(&self, positive: @(Self::area_spec(self) > 0)) -> (a: u32, @(a == Self::area_spec(self)));
        fn scaled(&self, by: u32, nonzero: @(by > 0)) -> (a: u32, @(a == Self::area_spec(self) * by && a >= by))
        where Self: Sized;
    }
    struct Rect { w: u32, h: u32 }
    impl Shape for Rect {
        logic fn area_spec(self: &Rect) -> Nat { self.w * self.h }
        logic fn area_is_bounded(s: &Rect, limit: Nat, h: @(Rect::area_spec(s) <= limit)) -> @(Rect::area_spec(s) < limit + 1) { _ }
        fn area(&self, positive: @(Rect::area_spec(self) > 0)) -> (a: u32, @(a == Rect::area_spec(self))) {
            (self.w * self.h, _)
        }
        fn scaled(&self, by: u32, nonzero: @(by > 0)) -> (a: u32, @(a == Rect::area_spec(self) * by && a >= by)) {
            (self.w * self.h * by, _)
        }
    }
    fn total<S: Shape>(s: &S) -> u32 { s.area(_).0 }
    fn run() -> (u32, u32) {
        let r = Rect { w: 2, h: 3 };
        (total(&r), Rect { w: 1, h: 2 }.scaled(2, _).0)
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
