//! expect: runtime=E0277 check=E0277
//! As a07, and the struct is read in logical code: an error in both forms.
//! In the conditional form it comes here, at the read, and names the field's type.
use ::locus::locus;
locus! {
    struct Server { sock: std::net::TcpListener, n: u32 }
    fn read(s: &Server) -> u32 { let c = logic! { snap(s) }; 1 }
    fn run() -> u32 { 1 }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
