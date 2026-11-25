//! expect: runtime=E0277 check=E0277
//! An opaque field is reached through the snapshot, `snap(s).sock`. Selected
//! on the place first, `s.sock`, it is a place of the foreign type, which
//! has no model.
use ::locus::locus;
locus! {
    struct Server { #[opaque] sock: std::net::TcpListener, n: u32 }
    fn read(s: &Server) -> u32 {
        let through_the_snapshot = logic! { snap(s).sock };
        let selected_first = logic! { s.sock };
        1
    }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
