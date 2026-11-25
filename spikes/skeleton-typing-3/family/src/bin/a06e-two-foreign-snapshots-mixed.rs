//! expect: runtime=E0308 check=E0308
//! Two foreign types are two opaque types: one where the other is wanted
//! is a type error. (Round 2 had one `Opaque` for both, and accepted this.)
use ::locus::locus;
locus! {
    struct Server {
        #[opaque] sock: std::net::TcpListener,
        #[opaque] peer: std::net::SocketAddr,
    }
    fn mix(s: &Server) -> u32 {
        let same = logic! { snap(s).sock === snap(s).sock };
        let mixed = logic! { snap(s).sock === snap(s).peer };
        1
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
