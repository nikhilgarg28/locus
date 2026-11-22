//! expect: runtime=ok check=ok
//! prints: run() => (6, "e2_trait_object_not_read::Scene_Snap<locus::Opaque<alloc::vec::Vec<alloc::boxed::Box<dyn e2_trait_object_not_read::Shape>>>, locus::Opaque<alloc::boxed::Box<dyn core::fmt::Debug>>, locus::Nat>")
//! Trait objects that logical code does not read: a `Box<dyn Shape>` for a
//! trait declared in `locus!`, and a `dyn Debug`. As parameters they need
//! nothing. As fields they are marked `#[opaque]`, and the struct is read.
use ::locus::locus;
locus! {
    trait Shape { fn area(&self) -> u32; }
    struct Square(u32);
    impl Shape for Square { fn area(&self) -> u32 { self.0 * self.0 } }
    struct Scene {
        #[opaque] shapes: Vec<Box<dyn Shape>>,
        #[opaque] label: Box<dyn std::fmt::Debug>,
        count: u32,
    }
    fn total(shape: Box<dyn Shape>, note: &dyn std::fmt::Debug, n: u32) -> (r: u32, @(n == n)) {
        let _ = format!("{note:?}");
        (shape.area() + n, _)
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (u32, &'static str) {
        let scene = Scene { shapes: vec![Box::new(Square(1))], label: Box::new(1u8), count: 1 };
        let s = logic! { snap(scene) };
        let (t, _) = total(Box::new(Square(2)), &"x", 2);
        (t, type_of(&s))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
