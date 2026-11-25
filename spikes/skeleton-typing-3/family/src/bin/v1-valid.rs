//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Prop", "locus::Prop", "locus::Prop", "locus::Prop", "locus::Prop")
//! Validity of captured values, typing only: `X::valid(s)` for a snapshot
//! `s` of an `X`. Lowering writes it for a struct, an enum and a generic
//! struct, from each field's; the library's are written by hand (`u8`: at
//! most 255; `[u8; 4]`: length 4 and each element's; an opaque field: true).
use ::locus::locus;
locus! {
    struct Pixel { r: u8, g: u8, tag: [u8; 4], #[opaque] from: std::net::SocketAddr }
    enum Shape { Dot, Circle(u8), Rect { w: u16, h: u16 }, Boxed(Box<Shape>) }
    struct Pair<T> { first: T, rest: Vec<T>, last: Option<Box<T>> }
    fn valid_of<T: Model>(x: &T) -> Prop { logic! { T::valid(snap(x)) } }
    logic fn all_valid<T: Model>(xs: Seq<Snap<T>>) -> Prop { forall |i: Nat| i < xs.len() ==> T::valid(xs[i]) }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
        let p = Pixel { r: 1, g: 2, tag: [0; 4], from: "127.0.0.1:1".parse().unwrap() };
        let s = Shape::Boxed(Box::new(Shape::Rect { w: 1, h: 2 }));
        let _ = (Shape::Dot, Shape::Circle(1));
        let pair = Pair { first: 1u8, rest: vec![2u8], last: None };
        let arr = [1u8, 2, 3, 4];
        let a = logic! { Pixel::valid(snap(p)) };
        let b = logic! { Shape::valid(snap(s)) };
        let generic = valid_of(&pair);
        let c = logic! { Pair::<u8>::valid(snap(pair)) && generic };
        let d = logic! { <[u8; 4]>::valid(snap(arr)) && <u8>::valid(snap(arr)[0]) };
        let e = logic! { match snap(s) { Shape::Circle(r) => <u8>::valid(r), Shape::Rect { w, h } => w <= 65535 && <u16>::valid(h), _ => True } };
        (type_of(&a), type_of(&b), type_of(&c), type_of(&d), type_of(&e))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
