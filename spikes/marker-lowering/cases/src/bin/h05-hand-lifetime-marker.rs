//! expect: runtime=reject check=reject
//! Plain Rust, written by hand: a marker with a lifetime parameter, which
//! `locus!` would not write (d02). This spike's `Logical` has the supertrait
//! `'static`, so rustc refuses it too.
use ::locus::Logical;
use std::marker::PhantomData;

#[derive(Clone, Copy)]
struct View<'a> {
    from: PhantomData<fn() -> &'a u32>,
}

impl<'a> Logical for View<'a> {
    fn __locus_marker() -> Self {
        View { from: PhantomData }
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
}
