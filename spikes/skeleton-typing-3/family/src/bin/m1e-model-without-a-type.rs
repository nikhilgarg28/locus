//! expect: runtime=annotate check=annotate
//! `model(s)` applies a model to a snapshot value, and the type whose model
//! is meant has to be written: `model::<Stack>(s)` or `Stack::model(s)`. A
//! snapshot type does not say what it is a snapshot of.
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32> }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
    }
    fn run() -> u32 {
        let s = Stack { items: vec![1] };
        let written = logic! { model::<Stack>(snap(s)).len() + Stack::model(snap(s)).len() };
        let bare = logic! { model(snap(s)).len() };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
