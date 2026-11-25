// expect: E0277. As c11, and then the struct is captured: the error names the field's type.
include!("../lib.rs");
struct Server { sock: std::net::TcpStream, n: u32 }
pub trait Has<'a> { type Snap: Ghost; }
impl<'a, T: Model> Has<'a> for T { type Snap = T::Snap; }
struct Server_Snap<S> { sock: S, n: Nat }
impl<S: Ghost> Ghost for Server_Snap<S> { fn ghost() -> Self { Server_Snap { sock: S::ghost(), n: Nat } } }
impl Model for Server where for<'a> std::net::TcpStream: Has<'a> {
    type Snap = Server_Snap<<std::net::TcpStream as Has<'static>>::Snap>; type Logic = Self::Snap;
}
fn read(s: &Server) { let _ = capture(s); }
fn main() {}
