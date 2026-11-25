//! expect: runtime=reject check=reject
//! What the one opaque snapshot costs: the snapshots of two unrelated
//! foreign types are one Rust type, so rustc accepts one where the other
//! is wanted. Locus's rules reject this; rustc cannot.
use ::locus::locus;
use std::collections::BTreeSet;
use std::time::Duration;
locus! {
    fn run() -> u32 {
        let set: BTreeSet<u32> = BTreeSet::new();
        let d = Duration::from_secs(1);
        let mut before = logic! { snap(set) };
        before = logic! { snap(d) };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
