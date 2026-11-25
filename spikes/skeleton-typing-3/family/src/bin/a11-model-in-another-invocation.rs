//! expect: runtime=error check=error
//! A model declared in another invocation than its type. Lowering sees no
//! declaration there, and refuses.
use ::locus::locus;
locus! {
    pub struct Stack { pub items: Vec<u32> }
}
locus! {
    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(s: Snap<Stack>) -> Seq<Nat> { s.items }
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
