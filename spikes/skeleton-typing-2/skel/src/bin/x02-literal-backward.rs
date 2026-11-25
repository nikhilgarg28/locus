//! expect: runtime=reject check=reject
//! `logic! { 0 }` with no annotation, and a later use by runtime code that
//! wants a `Nat`. Read forward, as Locus types logical code, the literal
//! is an `Int`, since nothing is expected of it where it stands, and the
//! later use is an error. rustc infers backward from that use.
use ::locus::locus;

locus! {
    fn takes_nat(n: Nat) -> u32 { 1 }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run() -> (&'static str, u32) {
        let zero = logic! { 0 };
        (type_of(&zero), takes_nat(zero))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
