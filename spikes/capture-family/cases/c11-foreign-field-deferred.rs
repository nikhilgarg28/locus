// expect: ok. The same with the clause behind a higher-ranked lifetime: the struct has no model, and is still a struct.
include!("../lib.rs");
struct Server { sock: std::net::TcpStream, n: u32 }
pub trait Has<'a> { type Snap: Ghost; }
impl<'a, T: Model> Has<'a> for T { type Snap = T::Snap; }
struct Server_Snap<S> { sock: S, n: Nat }
impl<S: Ghost> Ghost for Server_Snap<S> { fn ghost() -> Self { Server_Snap { sock: S::ghost(), n: Nat } } }
impl Model for Server where for<'a> std::net::TcpStream: Has<'a> {
    type Snap = Server_Snap<<std::net::TcpStream as Has<'static>>::Snap>; type Logic = Self::Snap;
}
fn uses_plain(s: &Server) -> u32 { s.n }            // the struct is still an ordinary Rust struct
fn main() {}
