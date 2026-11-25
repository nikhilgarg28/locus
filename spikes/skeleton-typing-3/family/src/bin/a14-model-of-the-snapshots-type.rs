//! expect: runtime=ok check=ok
//! prints: run() => ("a14_model_of_the_snapshots_type::Inner_Snap<locus::Nat>", "a14_model_of_the_snapshots_type::Inner_Snap<locus::Nat>", "locus::Prop")
//! SILENT. A declared model whose type is the snapshot's own type: `Inner`'s
//! model is its snapshot with `n` replaced by 0. Inside `Outer`, which has
//! no model, capture then project gives the field's snapshot (`n` as it is)
//! and select then capture gives the model (`n` is 0). The two have the SAME
//! type, rustc reports nothing, and `===` between them types.
use ::locus::locus;
locus! {
    struct Inner { n: u32 }
    impl Model for Inner {
        type Logic = Snap<Inner>;
        logic fn model(s: Snap<Inner>) -> Snap<Inner> { Inner_Snap { n: 0 as Nat } }
    }
    struct Outer { inner: Inner }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let outer = Outer { inner: Inner { n: 7 } };
        let whole = logic! { outer };
        let captured_then_projected = logic! { whole.inner };
        let selected_then_captured = logic! { outer.inner };
        let confused = logic! { captured_then_projected === selected_then_captured };
        (type_of(&captured_then_projected), type_of(&selected_then_captured), type_of(&confused))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
