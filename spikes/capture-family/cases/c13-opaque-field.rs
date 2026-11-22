// expect: E0308. A field marked opaque: two foreign types are two opaque types, and the last call mixes them.
include!("../lib.rs");
struct Server { sock: std::net::TcpStream, peer: std::net::SocketAddr, n: u32 }
#[derive(Clone, Copy)] struct Server_Snap { sock: Opaque<std::net::TcpStream>, peer: Opaque<std::net::SocketAddr>, n: Nat }
impl<T: ?Sized> Clone for Opaque<T> { fn clone(&self) -> Self { Opaque(PhantomData) } }
impl<T: ?Sized> Copy for Opaque<T> {}
impl Ghost for Server_Snap { fn ghost() -> Self { Server_Snap { sock: Ghost::ghost(), peer: Ghost::ghost(), n: Nat } } }
impl Model for Server { type Snap = Server_Snap; type Logic = Server_Snap; }
fn same<T>(_: T, _: T) {}
fn main() {
    let s = Server { sock: std::net::TcpStream::connect("127.0.0.1:1").unwrap_or_else(|_| std::process::exit(0)), peer: "127.0.0.1:1".parse().unwrap(), n: 0 };
    let c = capture(&s);
    same(c.sock, c.sock);
    same(c.sock, c.peer);        // two foreign types are two opaque types: this line must be the error
}
