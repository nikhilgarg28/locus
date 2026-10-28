// expect: check=ok run=ok
fn at(xs: &[u32], i: usize, ok: ::__locus::Proof![i < xs.len()]) -> u32 {
    xs[i]
}

fn indices(xs: &[u32]) -> Vec<(usize, ::__locus::Proof![i < xs.len()])> {
    let mut out: Vec<(usize, ::__locus::Proof![i < xs.len()])> = Vec::new(); ::__locus::observe!(&xs);
    for i in 0..xs.len() {
        out.push((i, ::__locus::proof!([] [])));
    }
    out
}

fn sum(xs: &[u32]) -> u32 {
    let checked = indices(xs);
    let mut total = 0;
    for (i, ok) in checked {
        total += at(xs, i, ok);
    }
    total
}

fn main() {
    let r = sum(&[1, 2, 3]);
    println!("sum(&[1, 2, 3]) = {r}");
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
