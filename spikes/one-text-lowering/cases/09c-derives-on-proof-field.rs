// expect: check=ok run=ok
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Witnessed {
    x: u32,
    ok: ::__locus::Proof![x > 0],
}

fn main() {
    let w = Witnessed { x: 1, ok: ::__locus::proof!([] []) };
    let mut set = std::collections::HashSet::new();
    set.insert(w);
    println!("w == w: {}, set.len() = {}, {:?}", w == w, set.len(), w);
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
