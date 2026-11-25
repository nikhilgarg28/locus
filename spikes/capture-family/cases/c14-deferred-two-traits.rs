// expect: ok. Two traits. The generated snapshot and default model are deferred; a declared model is unconditional and reads a field's snapshot; a struct with an unmarked foreign field is still a struct.
include!("../lib2.rs");
struct Stack { items: Vec<u32>, cap: usize }
#[derive(Clone, Copy)] struct Stack_Snap<F0, F1> { items: F0, cap: F1 }
impl<F0: Ghost, F1: Ghost> Ghost for Stack_Snap<F0, F1> { fn ghost() -> Self { Stack_Snap { items: F0::ghost(), cap: F1::ghost() } } }
unsafe impl Snapshot for Stack where for<'a> Vec<u32>: Has<'a>, for<'a> usize: Has<'a> {
    type Snap = Stack_Snap<<Vec<u32> as Has<'static>>::Snap, <usize as Has<'static>>::Snap>;
}
unsafe impl Model for Stack {                                   // the program's own model: unconditional
    type Logic = Seq<Nat>;
    fn model(s: <Stack as Snapshot>::Snap) -> Seq<Nat> { takes_nat(s.items.len()); takes_nat(s.cap); s.items }
}

struct Point { x: u32, y: u32 }
#[derive(Clone, Copy)] struct Point_Snap<F0, F1> { x: F0, y: F1 }
impl<F0: Ghost, F1: Ghost> Ghost for Point_Snap<F0, F1> { fn ghost() -> Self { Point_Snap { x: F0::ghost(), y: F1::ghost() } } }
unsafe impl Snapshot for Point where for<'a> u32: Has<'a> {
    type Snap = Point_Snap<<u32 as Has<'static>>::Snap, <u32 as Has<'static>>::Snap>;
}
unsafe impl Model for Point where for<'a> u32: Has<'a> {        // the generated default: deferred too
    type Logic = <Point as Snapshot>::Snap;
    fn model(s: <Point as Snapshot>::Snap) -> <Point as Snapshot>::Snap { s }
}

struct Server { sock: std::net::TcpStream, n: u32 }             // a field Locus cannot see, no mark
#[derive(Clone, Copy)] struct Server_Snap<F0, F1> { sock: F0, n: F1 }
impl<F0: Ghost, F1: Ghost> Ghost for Server_Snap<F0, F1> { fn ghost() -> Self { Server_Snap { sock: F0::ghost(), n: F1::ghost() } } }
unsafe impl Snapshot for Server where for<'a> std::net::TcpStream: Has<'a>, for<'a> u32: Has<'a> {
    type Snap = Server_Snap<<std::net::TcpStream as Has<'static>>::Snap, <u32 as Has<'static>>::Snap>;
}
unsafe impl Model for Server where for<'a> std::net::TcpStream: Has<'a>, for<'a> u32: Has<'a> {
    type Logic = <Server as Snapshot>::Snap;
    fn model(s: <Server as Snapshot>::Snap) -> <Server as Snapshot>::Snap { s }
}
fn port_of(s: &Server) -> u32 { s.n }                           // plain Rust over it still compiles

fn main() {
    let st = Stack { items: vec![1, 2], cap: 4 };
    takes_seq_nat(capture(&st));                                 // the declared model
    takes_nat(snap(&st).items.len());                            // the snapshot's field, with its own method
    let p = Point { x: 1, y: 2 };
    takes_nat(capture(&p).x);                                    // the default model's field is a Nat
    println!("ok {}", std::any::type_name_of_val(&capture(&p)));
}
