// expect: ok. The default model is the snapshot; a declared model replaces it; a wrapper with no model keeps its field's snapshot.
include!("../lib.rs");
struct Point { x: u32, y: i32 }
#[derive(Clone, Copy)] struct Point_Snap { x: <u32 as Model>::Snap, y: <i32 as Model>::Snap }
impl Ghost for Point_Snap { fn ghost() -> Self { Point_Snap { x: Ghost::ghost(), y: Ghost::ghost() } } }
impl Model for Point { type Snap = Point_Snap; type Logic = Point_Snap; }          // default: identity

struct Stack { items: Vec<u32>, cap: usize }
#[derive(Clone, Copy)] struct Stack_Snap { cap: Nat }
impl Ghost for Stack_Snap { fn ghost() -> Self { Stack_Snap { cap: Nat } } }
impl Model for Stack { type Snap = Stack_Snap; type Logic = Seq<Nat>; }           // declared model

struct Wrapper { s: Stack, n: u32 }
#[derive(Clone, Copy)] struct Wrapper_Snap { s: <Stack as Model>::Snap, n: <u32 as Model>::Snap }
impl Ghost for Wrapper_Snap { fn ghost() -> Self { Wrapper_Snap { s: Ghost::ghost(), n: Ghost::ghost() } } }
impl Model for Wrapper { type Snap = Wrapper_Snap; type Logic = Wrapper_Snap; }

fn main() {
    let p = Point { x: 1, y: -1 };
    let before = capture(&p);               // Point_Snap
    takes_nat(before.x);                    // an integer field of a captured struct is a Nat
    takes_nat(capture(&p.x));               // and so is the field read as a place: the two agree
    let w = Wrapper { s: Stack { items: vec![], cap: 4 }, n: 0 };
    takes_seq_nat(capture(&w.s));           // select the field, then capture: the model
    let ws = capture(&w).s;                 // capture, then project: the field's snapshot
    takes_nat(ws.cap);
    takes_seq_nat(model_of::<Stack>(ws));   // and the model is reached by writing it
    println!("{} | {}", name(&capture(&w.s)), name(&ws));
}
