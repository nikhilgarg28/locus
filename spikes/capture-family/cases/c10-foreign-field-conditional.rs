// expect: E0277. The same with a plain `where` clause: rustc checks it at the declaration.
include!("../lib.rs");
struct Server { sock: std::net::TcpStream, n: u32 }
struct Server_Snap<S> { sock: S, n: Nat }
impl<S: Ghost> Ghost for Server_Snap<S> { fn ghost() -> Self { Server_Snap { sock: S::ghost(), n: Nat } } }
impl Model for Server where std::net::TcpStream: Model {
    type Snap = Server_Snap<<std::net::TcpStream as Model>::Snap>; type Logic = Self::Snap;
}
fn main() {}
