// expect: E0277. A struct whose generated snapshot names the snapshot of a field type that has none.
include!("../lib.rs");
struct Server { sock: std::net::TcpStream, n: u32 }
struct Server_Snap { sock: <std::net::TcpStream as Model>::Snap, n: Nat }
fn main() {}
