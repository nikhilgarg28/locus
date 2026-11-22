// Probe q1: the view of a logical enum reached through the type alone, by a
// library alias of a projection, and what E0004 prints for several namings.
#![allow(dead_code, non_camel_case_types)]
use std::marker::PhantomData;
pub trait Matchable { type Variants; }
pub type Variants<T> = <T as Matchable>::Variants;
pub fn variants<T: Matchable>(_: T) -> T::Variants { panic!() }
#[derive(Clone, Copy)] pub struct Nat;

mod shapes {
    use super::*;
    #[derive(Clone, Copy)] pub struct Tree {}
    // (a) the view inside an anonymous constant, under the type's own name
    const _: () = {
        pub enum Tree { Leaf, Node(self::Tree, Nat, self::Tree) }
        impl Matchable for self::Tree { type Variants = Tree; }
    };
    // (b) a generic one
    pub struct Maybe<T> { p: PhantomData<fn() -> T> }
    impl<T> Clone for Maybe<T> { fn clone(&self) -> Self { *self } }
    impl<T> Copy for Maybe<T> {}
    const _: () = {
        pub enum Maybe<T> { None, Some(T) }
        impl<T> Matchable for self::Maybe<T> { type Variants = Maybe<T>; }
    };
}
mod user {
    use super::{variants, Variants, Nat};
    use super::shapes::Tree;          // imported by name: nothing else comes
    use super::shapes::Maybe;
    pub fn size(t: Tree) -> u32 {
        match variants(t) {
            Variants::<Tree>::Leaf => 0,
            Variants::<Tree>::Node(l, _, r) => size(l) + size(r),
        }
    }
    pub fn get(m: Maybe<Nat>) -> u32 {
        match variants(m) { Variants::<Maybe<_>>::None => 0, Variants::<Maybe<_>>::Some(_n) => 1 }
    }
    #[cfg(missing)]
    pub fn missing(t: Tree) -> u32 {
        match variants(t) { Variants::<Tree>::Node(..) => 1 }
    }
}
fn main() { println!("q1 ok"); }
