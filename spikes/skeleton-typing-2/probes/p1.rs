// Probe 1: the core encodings, by hand.
#![allow(dead_code, unused_variables, non_snake_case)]
use std::marker::PhantomData;

pub trait Value { type V; }
pub trait Logical: Copy + 'static + Value<V = Self> { fn marker() -> Self; }
macro_rules! zst { ($($n:ident),*) => {$( #[derive(Clone, Copy, Debug)] pub struct $n; impl Value for $n { type V = $n; } impl Logical for $n { fn marker() -> Self { $n } } )*} }
zst!(Nat, Int, Bool, Prop, Erased);
pub struct Seq<T>(PhantomData<fn() -> T>);
impl<T> Clone for Seq<T> { fn clone(&self) -> Self { *self } } impl<T> Copy for Seq<T> {}
impl<T: 'static> Value for Seq<T> { type V = Self; }
impl<T: Logical> Logical for Seq<T> { fn marker() -> Self { Seq(PhantomData) } }
impl<T> Seq<T> { fn __logic_len(self) -> Nat { stub() } }

// runtime types: value = model
impl Value for u32 { type V = Nat; }
impl Value for u8 { type V = Nat; }
impl Value for usize { type V = Nat; }
impl Value for bool { type V = Bool; }
impl<T: Value> Value for Vec<T> { type V = Seq<T::V>; }
impl<'a, T: Value + ?Sized> Value for &'a T { type V = T::V; }
impl<'a, T: Value + ?Sized> Value for &'a mut T { type V = T::V; }

// the recursion wrapper
pub struct Rec<T>(PhantomData<fn() -> T>);
impl<T> Value for Rec<T> { type V = T; }

pub fn stub<T>() -> T { panic!("stub") }
pub fn value<T: Value + ?Sized>(_: &T) -> T::V { stub() }

// argument position: the expected type selects the route
pub struct ViaSelf; pub struct ViaModel; pub struct ViaRec; pub struct ViaWiden;
pub trait Runtime: Value {}            // runtime types with a model
impl Runtime for u32 {} impl Runtime for u8 {} impl Runtime for usize {} impl Runtime for bool {}
impl<T: Runtime> Runtime for Vec<T> {}
impl<'a, T: Runtime + ?Sized> Runtime for &'a T {}
impl<'a, T: Runtime + ?Sized> Runtime for &'a mut T {}
impl<T> Runtime for Rec<T> {}
pub trait ArgAs<P, Via> {}
impl<T> ArgAs<T, ViaSelf> for T {}
impl<T: Runtime + ?Sized> ArgAs<T::V, ViaModel> for T {}
impl<T> ArgAs<Rec<T>, ViaRec> for T {}
impl ArgAs<Int, ViaWiden> for Nat {}
pub fn arg<T: ArgAs<P, Via> + ?Sized, P, Via>(_: &T) -> P { stub() }

pub fn typed<G: Logical>(_: impl FnOnce() -> G) -> G { G::marker() }

// literals
pub trait Literal {} impl Literal for Nat {} impl Literal for Int {}
pub fn lit<L: Literal>(_: u128) -> L { stub() }
pub fn lit_like<L: Literal>(_: &L, _: u128) -> L { stub() }

pub trait AddLogic<Rhs = Self> { type Output; }
impl AddLogic for Nat { type Output = Nat; } impl AddLogic for Int { type Output = Int; }
pub fn add<A: AddLogic<B>, B>(_: A, _: B) -> A::Output { stub() }
pub trait EqLogic {} impl EqLogic for Nat {} impl EqLogic for Int {} impl EqLogic for Bool {}
pub fn eq<T: EqLogic>(_: T, _: T) -> Bool { stub() }
pub fn same<T>(_: T, _: T) -> Prop { stub() }

// a recursive logical enum with real variants
#[derive(Clone, Copy)]
pub enum Tree { Leaf, Node(Rec<Tree>, Nat, Rec<Tree>) }
impl<T> Clone for Rec<T> { fn clone(&self) -> Self { *self } } impl<T> Copy for Rec<T> {}
impl Value for Tree { type V = Tree; }
impl Logical for Tree { fn marker() -> Self { Tree::Leaf } }

fn double(n: Nat) -> Nat { stub() }
fn size(t: Tree) -> Nat {
    // match t { Tree::Leaf => 0, Tree::Node(left, _, right) => size(left) + 1 + size(right) }
    match value(&t) {
        Tree::Leaf => lit(0),
        Tree::Node(left, _, right) => add({ let l = size(arg(&left)); add(l, lit_like(&l, 1)) }, size(arg(&right))),
    }
}
fn generic_id<T>(t: T) -> T { arg(&t) }
fn generic_is_max<T: Copy>(m: T, a: T) -> Prop { same(arg::<_, T, _>(&m), arg(&a)) }
fn takes_int(i: Int) {}

fn main() {
    let v = vec![1u32, 2];
    let n: u8 = 3;
    let before = typed(|| value(&v).__logic_len());            // logic! { v.len() }
    let twice = typed(|| double(value(&v).__logic_len()));     // logic! { double(v.len()) }
    let b2 = typed(|| value(&n));                              // logic! { n }
    let d2 = typed(|| double(arg(&n)));                        // logic! { double(n) }
    let d3 = typed(|| double(arg(&before)));                   // logic! { double(before) }
    let leaf = typed(|| Tree::Leaf);
    let node = typed(|| Tree::Node(arg(&leaf), lit(0), arg(&leaf)));
    let sz = typed(|| size(arg(&node)));
    let w = typed(|| { takes_int(arg(&before)); lit::<Int>(0) });
    let p = typed(|| eq(value(&before), value(&v).__logic_len()));
    println!("{} {} {} {} {}", std::any::type_name_of_val(&before), std::any::type_name_of_val(&twice), std::any::type_name_of_val(&b2), std::any::type_name_of_val(&d2), std::any::type_name_of_val(&sz));
    println!("sizes {} {}", std::mem::size_of::<Tree>(), std::mem::size_of_val(&node));
    drop(v);
}
