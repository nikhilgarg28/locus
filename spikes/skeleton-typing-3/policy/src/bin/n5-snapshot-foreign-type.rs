//! expect: runtime=ok check=ok
//! prints: run() => ("locus::Opaque", "locus::Opaque", 1)
//! A concrete type that nothing declares a snapshot for (a type of a crate
//! that Locus never lowered: here `BTreeSet` and `Duration`). RFC 0002
//! makes its snapshot opaque. `snap(x)` gets an opaque one; the type
//! `Snap<BTreeSet<u32>>`, written out, has no meaning for rustc.
use ::locus::locus;
use std::collections::BTreeSet;
use std::time::Duration;
locus! {
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
    fn named(s: Snap<BTreeSet<u32>>) -> u32 { 1 }
    fn run() -> (&'static str, &'static str, u32) {
        let set: BTreeSet<u32> = BTreeSet::new();
        let d = Duration::from_secs(1);
        let before = logic! { snap(set) };
        (type_of(&before), type_of(&logic! { snap(d) }), named(before))
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
