// expect: ok
// A program's own logical types and traits, beside the library's and the standard library's.
extern crate lg;
use lg::*;
use std::marker::PhantomData;
use std::ops::Add;

#[derive(Clone, Copy)] struct Version;                       // #[derive(Logical)] struct Version { .. }
impl Logical for Version { fn marker() -> Self { Version } }
#[derive(Clone, Copy)] struct List;                          // a logical type that contains itself
impl Logical for List { fn marker() -> Self { List } }
struct Tree<T>(PhantomData<fn() -> T>);                      // a generic one that contains itself
impl<T> Clone for Tree<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Tree<T> {}
impl<T: Logical> Logical for Tree<T> { fn marker() -> Self { Tree(PhantomData) } }

impl OrdLogic for Version {}                                 // the library's trait, the program's type
impl<T: OrdLogic> OrdLogic for Tree<T> {}
impl Add for Version { type Output = Version; fn add(self, _: Version) -> Version { Version } }
impl From<Int> for Version { fn from(_: Int) -> Version { Version } }
impl From<Version> for Int { fn from(_: Version) -> Int { Int } }   // a foreign self type, a local argument

trait Size: Logical { fn size(self) -> Nat { Nat } }         // the program's trait, the library's types
impl Size for Int {}
impl Size for Seq<Nat> {}
impl Size for List {}

trait Pre: Logical { fn le(_a: Self, _b: Self) -> Bool { Bool } }   // a second trait with a member `le`
impl Pre for Int {}

trait Sortable { fn sorted(self) -> Prop; }
impl<T: OrdLogic> Sortable for Seq<T> { fn sorted(self) -> Prop { Prop } }   // an implementation with a bound

trait AddLogic<Rhs: Logical>: Logical { type Output: Logical; }      // a trait with a type parameter
impl AddLogic<Nat> for Int { type Output = Int; }
impl AddLogic<Int> for Nat { type Output = Int; }

fn main() {
    let _ = Version::le(Version, Version);                   // `Version` has only `OrdLogic::le`: not ambiguous
    let _ = <Seq<Version> as OrdLogic>::le(Seq::marker(), Seq::marker());
    let _ = <Tree<Seq<Nat>> as OrdLogic>::le(Tree::marker(), Tree::marker());
    let _: Version = Version + Version;
    let _: Int = Version.into();
    let _: Nat = Seq::<Nat>::marker().size();
    let _: Prop = Seq::<Version>::marker().sorted();
    let _: <Int as AddLogic<Nat>>::Output = Int;
    println!("resolved by rustc: user and library traits on user and library logical types");
}
