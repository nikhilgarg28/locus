//! expect: runtime=error check=error
//! A logic function's parameters are logical types: RFC 0002's own
//! `logic fn model(stack: &Stack)` and `logic fn sorted(xs: &[u32])` are
//! errors from lowering, which say what to write.
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32> }
    logic fn top(stack: &Stack) -> Nat { stack.items.last() }
    logic fn sorted(xs: &[u32]) -> Prop { forall |i: Nat, j: Nat| i <= j && j < xs.len() ==> xs[i] <= xs[j] }
    impl Stack {
        logic fn room(self) -> Nat { self.items.len() }
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
