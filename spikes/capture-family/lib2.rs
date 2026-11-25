// Two traits: the snapshot (generated, and deferred for a struct's fields), and the model.
use std::marker::PhantomData;
pub trait Ghost { fn ghost() -> Self; }
#[derive(Clone, Copy)] pub struct Nat;
pub struct Seq<T>(PhantomData<fn() -> T>);
impl Ghost for Nat { fn ghost() -> Self { Nat } }
impl<T> Ghost for Seq<T> { fn ghost() -> Self { Seq(PhantomData) } }
impl<T> Clone for Seq<T> { fn clone(&self) -> Self { Seq(PhantomData) } }
impl<T> Copy for Seq<T> {}
impl<T> Seq<T> { pub fn len(self) -> Nat { Nat } }

#[diagnostic::on_unimplemented(message = "`{Self}` cannot be read in logical code", note = "mark a field of this type `#[opaque]`, or for a type parameter add the bound `{Self}: Model`")]
pub unsafe trait Snapshot { type Snap: Ghost; }
pub unsafe trait Model: Snapshot { type Logic: Ghost; fn model(s: Self::Snap) -> Self::Logic; }
// The deferral: a bound through this trait, behind a higher-ranked lifetime, is not checked at a declaration.
pub trait Has<'a> { type Snap: Ghost; }
impl<'a, T: ?Sized + Snapshot> Has<'a> for T { type Snap = T::Snap; }

macro_rules! plain { ($($t:ty),*) => { $(unsafe impl Snapshot for $t { type Snap = Nat; } unsafe impl Model for $t { type Logic = Nat; fn model(s: Nat) -> Nat { s } })* } }
plain!(u8, u32, u64, usize);
unsafe impl<T: Snapshot> Snapshot for Vec<T> { type Snap = Seq<T::Snap>; }
unsafe impl<T: Model> Model for Vec<T> { type Logic = Seq<T::Logic>; fn model(_: Seq<T::Snap>) -> Seq<T::Logic> { Ghost::ghost() } }

pub fn snap<T: ?Sized + Snapshot>(_: &T) -> T::Snap { Ghost::ghost() }
pub fn capture<T: ?Sized + Model>(_: &T) -> T::Logic { Ghost::ghost() }
pub fn takes_nat(_: Nat) {}
pub fn takes_seq_nat(_: Seq<Nat>) {}
