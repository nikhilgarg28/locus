// expect: check=E0282 run=ok
// The probe written plainly: its type argument is never inferred.
#[allow(non_camel_case_types)] struct __locus_List<T>(::std::marker::PhantomData<T>); impl<T> ::__locus::Proj for __locus_List<T> { type Out = ::__locus::ErasedData; }
type List<T> = <__locus_List<T> as ::__locus::Proj>::Out;
#[allow(non_snake_case)] impl<T> __locus_List<T> { fn Nil() -> List<T> { ::__locus::stub() }
    fn Cons(_: T, _: List<T>) -> List<T> { ::__locus::stub() } }
//

fn empty() -> u32 {
    let l = ::__locus::data!([] [__locus_List::Nil] List::Nil);
    0
}

fn main() {
    println!("empty() = {}", empty());
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
