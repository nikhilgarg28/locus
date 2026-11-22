// expect: E0277. As c14, and a model is declared for the struct with the unmarked foreign field: the error is at the model.
include!("../lib2.rs");
struct Server { sock: std::net::TcpStream, n: u32 }
#[derive(Clone, Copy)] struct Server_Snap<F0, F1> { sock: F0, n: F1 }
impl<F0: Ghost, F1: Ghost> Ghost for Server_Snap<F0, F1> { fn ghost() -> Self { Server_Snap { sock: F0::ghost(), n: F1::ghost() } } }
unsafe impl Snapshot for Server where for<'a> std::net::TcpStream: Has<'a>, for<'a> u32: Has<'a> {
    type Snap = Server_Snap<<std::net::TcpStream as Has<'static>>::Snap, <u32 as Has<'static>>::Snap>;
}
unsafe impl Model for Server { type Logic = Nat; fn model(s: <Server as Snapshot>::Snap) -> Nat { s.n } }
fn main() {}
