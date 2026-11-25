//! expect: runtime=E0277 check=E0277
//! A field of a type that Locus knows nothing of, with no mark: in the
//! default (plain) form, an error where the struct is declared. With
//! LOCUS_SPIKE_DEFERRED=1 (the conditional form of capture-family c11) the
//! struct compiles, and is an ordinary Rust struct.
use ::locus::locus;
locus! {
    struct Server { sock: std::net::TcpListener, n: u32 }
    fn port(s: &Server) -> u32 { s.n }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
