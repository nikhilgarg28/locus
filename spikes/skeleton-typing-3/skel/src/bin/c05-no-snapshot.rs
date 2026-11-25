//! expect: runtime=ok check=ok
//! prints: run() => 1
//! A type with no snapshot implementation: one of the standard library
//! that the spike's library does not list. By RFC 0002 every type has a
//! snapshot, opaque unless said otherwise. A family has no default member.
use ::locus::locus;

locus! {
    use std::collections::BTreeSet;

    fn run(set: &BTreeSet<u32>) -> usize {
        let before = logic! { snap(set) };
        let same: @(snap(set) === before) = _;
        set.len()
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&[1].into()));
}
