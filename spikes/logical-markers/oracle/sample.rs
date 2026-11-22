//! What a checking program could hold under this lowering: logical types as zero-sized types of
//! their own, and implementations on them. No value of these types is made, and nothing here asks
//! a question: the driver builds each type itself and asks rustc afterwards.
#![allow(dead_code)]
use std::marker::PhantomData;

pub trait Logical {}
pub struct Erased;
pub struct Int;
pub struct Nat;
pub struct Bool;
pub struct Version;
pub struct Seq<T>(PhantomData<fn() -> T>);
impl Logical for Erased {}
impl Logical for Int {}
impl Logical for Nat {}
impl Logical for Bool {}
impl Logical for Version {}
impl<T: Logical> Logical for Seq<T> {}

pub trait OrdLogic: Logical {
    fn le(a: Self, b: Self) -> Bool;
}
impl OrdLogic for Int { fn le(_: Int, _: Int) -> Bool { Bool } }
impl OrdLogic for Nat { fn le(_: Nat, _: Nat) -> Bool { Bool } }
impl OrdLogic for Version { fn le(_: Version, _: Version) -> Bool { Bool } }
impl<T: OrdLogic> OrdLogic for Seq<T> { fn le(_: Seq<T>, _: Seq<T>) -> Bool { Bool } }

pub trait Family: Logical { type Out: Logical; }
impl Family for Int { type Out = Int; }
impl Family for Nat { type Out = Erased; }
impl<T: Family> Family for Seq<T> { type Out = Seq<T::Out>; }

pub fn generic<T: OrdLogic>() {}
