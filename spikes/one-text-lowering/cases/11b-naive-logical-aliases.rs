// expect: check=E0116,E0091 run=E0116,E0091
// The naive lowering of 11-locus-items.lc's #[derive(Logical)] types.
type Point = ::__locus::ErasedData;

impl Point {
    fn origin() -> Point {
        ::__locus::stub()
    }
}

type List<T> = ::__locus::ErasedData;

fn main() {}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
