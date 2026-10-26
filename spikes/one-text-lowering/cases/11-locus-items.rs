// expect: check=ok run=ok
fn double(n: Nat) -> Nat {
    ::__locus::stub() // logical body: n + n
}

#[allow(non_snake_case)] fn Small(n: Nat) -> Prop {
    ::__locus::stub() // logical body: n < 10
}

#[allow(non_camel_case_types)] struct __locus_Point; impl ::__locus::Proj for __locus_Point { type Out = ::__locus::ErasedData; }
type Point = <__locus_Point as ::__locus::Proj>::Out;
    // logical field x: Int,
    // logical field y: Int,
//

impl __locus_Point {
    fn origin() -> Point {
        ::__locus::stub() // logical body: Point { x: 0, y: 0 }
    }
}

#[allow(non_camel_case_types)] struct __locus_List<T>(::std::marker::PhantomData<T>); impl<T> ::__locus::Proj for __locus_List<T> { type Out = ::__locus::ErasedData; }
type List<T> = <__locus_List<T> as ::__locus::Proj>::Out;
#[allow(non_snake_case)] impl<T> __locus_List<T> { fn Nil() -> List<T> { ::__locus::stub() }
    fn Cons(_: T, _: List<T>) -> List<T> { ::__locus::stub() } }
//

pub fn halve_even(n: u32, even: ::__locus::Proof![n % 2 == 0]) -> (u32, ::__locus::Proof![out * 2 == n]) {
    (n / 2, ::__locus::proof!([] []))
}

fn use_them() -> u32 {
    let d = ::__locus::data!([] [double] double(3));
    let s = ::__locus::data!([&d] [Small] Small(d));
    let o: Point = ::__locus::data!([] [__locus_Point::origin] Point::origin());
    let l: List<Nat> = ::__locus::data!([] [__locus_List::<::__locus::ErasedData>::Nil] List::Nil);
    let (half, exact) = halve_even(8, ::__locus::proof!([] []));
    half
}

fn main() {
    println!("use_them() = {}", use_them());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
