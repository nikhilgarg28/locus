// expect: E0119. A declared model beside the generated default is a conflict.
include!("../lib.rs");
struct Stack { cap: usize }
#[derive(Clone, Copy)] struct Stack_Snap { cap: Nat }
impl Ghost for Stack_Snap { fn ghost() -> Self { Stack_Snap { cap: Nat } } }
impl Model for Stack { type Snap = Stack_Snap; type Logic = Stack_Snap; }          // generated default
impl Model for Stack { type Snap = Stack_Snap; type Logic = Seq<Nat>; }           // the program's own
fn main() {}
