//! expect: runtime=ok check=ok
//! prints: run() => ("a06_opaque_field::Server_Snap<locus::Opaque<std::net::tcp::TcpListener>, locus::Opaque<core::net::socket_addr::SocketAddr>, locus::Opaque<&str>, locus::Nat>", "locus::Opaque<std::net::tcp::TcpListener>", "locus::Prop")
//! Fields marked `#[opaque]`: each has the snapshot `Opaque<ItsType>`, with
//! the lifetimes that lowering sees written `'static`, so the snapshot
//! outlives the borrow in `name`.
use ::locus::locus;
locus! {
    struct Server<'a> {
        #[opaque] sock: std::net::TcpListener,
        #[opaque] peer: std::net::SocketAddr,
        #[opaque] name: &'a str,
        n: u32,
    }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn run() -> (&'static str, &'static str, &'static str) {
        let name = String::from("here");
        let s = Server { sock: std::net::TcpListener::bind("127.0.0.1:0").unwrap(), peer: "127.0.0.1:1".parse().unwrap(), name: &name, n: 0 };
        let c = logic! { s };
        let same = logic! { c.sock === c.sock };
        drop(s);
        drop(name);
        (type_of(&c), type_of(&logic! { c.sock }), type_of(&same))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
