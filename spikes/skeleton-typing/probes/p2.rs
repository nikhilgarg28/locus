// Probe 2: sizes and exhaustiveness of the enum encodings; supertrait projection; recursion.
#![allow(dead_code, unused_variables)]
use std::marker::PhantomData;
use std::convert::Infallible;
#[derive(Clone, Copy)] pub struct Nat;
#[derive(Clone, Copy)] pub enum Light { Red, Green }                       // real variants
#[derive(Clone, Copy)] pub enum Light0 { Red, Green(Infallible) }          // all but one uninhabited
pub struct Tree { __l: PhantomData<fn() -> ()> }                            // nominal, no payload
pub enum TreeView { Leaf, Node(Tree, Nat, Tree) }                           // its variants, beside it
pub enum ListView<T> { Nil, Cons(T, List<T>), __Phantom(PhantomData<fn() -> T>, Infallible) }
pub struct List<T> { __l: PhantomData<fn() -> T> }
pub trait HasView<V> {}
impl HasView<TreeView> for Tree {}
impl<T> HasView<ListView<T>> for List<T> {}
pub fn stub<T>() -> T { panic!() }
pub fn view<T: HasView<V>, V>(_: T) -> V { stub() }
fn size(t: Tree) -> u8 { match view(t) { TreeView::Leaf => 0, TreeView::Node(l, _, r) => 1 } }
fn len<T>(t: List<T>) -> u8 { match view(t) { ListView::Nil => 0, ListView::Cons(_, rest) => 1 + len(rest) } }
fn half(l: Light0) -> u8 { match l { Light0::Red => 0 } }                   // accepted: rustc no longer asks for Green

pub trait Model { type Logic; }
pub trait Logical: Copy + 'static + Model<Logic = Self> {}
pub fn model_of<T: Model + ?Sized>(_: &T) -> T::Logic { stub() }
fn generic<T: Logical>(x: T) -> T { model_of(&x) }                          // normalizes through the supertrait
fn main() {
    println!("Light {} Light0 {} Tree {} TreeView {}", size_of::<Light>(), size_of::<Light0>(), size_of::<Tree>(), size_of::<TreeView>());
}
