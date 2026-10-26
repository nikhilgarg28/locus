// expect: check=E0308 run=E0308
// `ok` is bound by a pattern from a runtime value, so it is not lexically
// ghost, and `ok && ok` is left as runtime code.
fn twice(checked: Vec<(usize, ::__locus::Proof![i < 3])>) -> usize {
    let mut n = 0;
    for (i, ok) in checked {
        let both = ok && ok; //~ error: Locus should say: write logic! { ok && ok }
        n += i;
    }
    n
}

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
