//! expect: runtime=E0308 check=E0308
//! Capture then project where select then capture was meant: the field's
//! snapshot where its model is wanted. A type error, never silent, as long
//! as the model's type is not the snapshot's (see a14).
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32>, cap: usize }
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
    }
    struct Wrapper { s: Stack }
    logic fn takes_seq(xs: Seq<Nat>) -> Seq<Nat> { xs }
    fn run() -> u32 {
        let w = Wrapper { s: Stack { items: vec![1, 2], cap: 4 } };
        let whole = logic! { w };
        let wrong = logic! { takes_seq(whole.s) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
