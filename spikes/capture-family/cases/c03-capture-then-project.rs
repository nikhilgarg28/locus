// expect: E0308. Capture-then-project where select-then-capture was meant is a type error.
include!("../lib.rs");
struct Stack { cap: usize }
#[derive(Clone, Copy)] struct Stack_Snap { cap: Nat }
impl Ghost for Stack_Snap { fn ghost() -> Self { Stack_Snap { cap: Nat } } }
impl Model for Stack { type Snap = Stack_Snap; type Logic = Seq<Nat>; }
struct Wrapper { s: Stack }
#[derive(Clone, Copy)] struct Wrapper_Snap { s: <Stack as Model>::Snap }
impl Ghost for Wrapper_Snap { fn ghost() -> Self { Wrapper_Snap { s: Ghost::ghost() } } }
impl Model for Wrapper { type Snap = Wrapper_Snap; type Logic = Wrapper_Snap; }
fn main() { let w = Wrapper { s: Stack { cap: 1 } }; takes_seq_nat(capture(&w).s); }
