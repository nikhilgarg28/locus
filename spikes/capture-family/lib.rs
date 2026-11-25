// What a lowering would have to produce, written by hand. One family, by trait impls.
use std::marker::PhantomData;
pub trait Ghost { fn ghost() -> Self; }
#[derive(Clone, Copy)] pub struct Nat;
#[derive(Clone, Copy)] pub struct Int;
#[derive(Clone, Copy)] pub struct Proof;
pub struct Seq<T>(PhantomData<fn() -> T>);
pub struct Opaque<T: ?Sized>(PhantomData<fn() -> T>);
impl Ghost for Nat { fn ghost() -> Self { Nat } }
impl Ghost for Int { fn ghost() -> Self { Int } }
impl<T> Ghost for Seq<T> { fn ghost() -> Self { Seq(PhantomData) } }
impl<T: ?Sized> Ghost for Opaque<T> { fn ghost() -> Self { Opaque(PhantomData) } }
impl<T> Clone for Seq<T> { fn clone(&self) -> Self { Seq(PhantomData) } }
impl<T> Copy for Seq<T> {}
impl<T> Seq<T> { pub fn len(self) -> Nat { Nat } }

#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be read in logical code",
    label = "this type has no snapshot that Locus knows",
    note = "for a type parameter, add the bound `{Self}: Model`",
    note = "for a type of a crate that Locus does not check, mark the field `#[opaque]` or wrap the type in a struct of your own"
)]
pub trait Model { type Snap: Ghost; type Logic: Ghost; }

macro_rules! plain { ($($t:ty => $l:ty),*) => { $(impl Model for $t { type Snap = $l; type Logic = $l; })* } }
plain!(u8 => Nat, u32 => Nat, u64 => Nat, usize => Nat, i32 => Int, i64 => Int);
impl<'a, T: ?Sized + Model> Model for &'a T { type Snap = T::Snap; type Logic = T::Logic; }
impl<'a, T: ?Sized + Model> Model for &'a mut T { type Snap = T::Snap; type Logic = T::Logic; }
impl<T: ?Sized + Model> Model for Box<T> { type Snap = T::Snap; type Logic = T::Logic; }
impl<T: Model> Model for [T] { type Snap = Seq<T::Snap>; type Logic = Seq<T::Logic>; }

pub fn snap<T: ?Sized + Model>(_: &T) -> T::Snap { <T::Snap as Ghost>::ghost() }
pub fn capture<T: ?Sized + Model>(_: &T) -> T::Logic { <T::Logic as Ghost>::ghost() }
pub fn model_of<T: Model>(_: T::Snap) -> T::Logic { <T::Logic as Ghost>::ghost() }
pub fn takes_nat(_: Nat) {}
pub fn takes_seq_nat(_: Seq<Nat>) {}
pub fn name<T>(_: &T) -> &'static str { std::any::type_name::<T>() }
