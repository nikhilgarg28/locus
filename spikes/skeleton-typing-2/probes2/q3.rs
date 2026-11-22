// Probe q3 (P5): a method call on a runtime place finds a logic function
// declared on the runtime type first, then one on its model. The name of
// the method is a type (const generic chars), a marker trait `Declares`
// says which types declare a logic function of that name, and method
// probing's by-value-before-autoref order chooses.
#![allow(dead_code, unused_variables, non_snake_case)]
use std::marker::PhantomData;
fn stub<T>() -> T { panic!() }
#[derive(Clone, Copy, Debug)] pub struct Nat;
pub struct Seq<T>(PhantomData<fn() -> T>);
impl<T> Clone for Seq<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Seq<T> {}
#[diagnostic::on_unimplemented(message = "`{Self}` has no model, so a value of it cannot be used as a value in logical code")]
pub trait Model { type Logic; }
impl Model for Nat { type Logic = Nat; }
impl<T> Model for Seq<T> { type Logic = Seq<T>; }
impl Model for u32 { type Logic = Nat; }
impl<T: Model> Model for Vec<T> { type Logic = Seq<T::Logic>; }
impl<T: Model + ?Sized> Model for &T { type Logic = T::Logic; }

/// A name, as a type: `N<'l', N<'e', N<'n', ()>>>`.
pub struct N<const C: char, Rest>(PhantomData<Rest>);
/// `Self` declares a logic function of this name that takes `self`.
pub trait Declares<Name> { type Owner; }
impl<Name, T: Declares<Name> + ?Sized> Declares<Name> for &T { type Owner = T::Owner; }
impl<Name, T: Declares<Name> + ?Sized> Declares<Name> for Box<T> { type Owner = T::Owner; }
pub struct Receiver<Name, T: ?Sized>(PhantomData<(fn() -> Name, fn() -> T)>);
pub fn receiver<Name, T: ?Sized>(_: &T) -> Receiver<Name, T> { stub() }
pub trait OnTheType { type Out; fn __locus_receiver(&self) -> Self::Out; }
impl<Name, T: Declares<Name> + ?Sized> OnTheType for Receiver<Name, T> {
    type Out = T::Owner;
    fn __locus_receiver(&self) -> T::Owner { stub() }
}
pub trait OnTheModel { type Out; fn __locus_receiver(&self) -> Self::Out; }
impl<Name, T: Model + ?Sized> OnTheModel for &Receiver<Name, T> {
    type Out = T::Logic;
    fn __locus_receiver(&self) -> T::Logic { stub() }
}

// impl<T> Seq<T> { logic fn len(self) -> Nat; }
impl<T> Seq<T> { fn __logic_len(self) -> Nat { stub() } }
// A runtime type with a hand-written model, and a logic function of its own.
struct Stack { items: Vec<u32>, cap: usize }
impl Model for Stack { type Logic = Seq<Nat>; }
impl Stack { fn __logic_room(self) -> Nat { stub() } }
impl Declares<N<'r', N<'o', N<'o', N<'m', ()>>>>> for Stack { type Owner = Stack; }
// A runtime type with no model.
struct Plain { n: u32 }
impl Plain { fn __logic_twice(self) -> Nat { stub() } }
impl Declares<N<'t', N<'w', N<'i', N<'c', N<'e', ()>>>>>> for Plain { type Owner = Plain; }

fn takes_nat(_: Nat) {}
fn user<T: Model<Logic = Seq<Nat>>>(s: &Stack, b: Box<Stack>, v: Vec<u32>, p: Plain, t: T) {
    let _ = || {
        // s.room(): on the runtime type, through the reference
        takes_nat((&receiver::<N<'r', N<'o', N<'o', N<'m', ()>>>>, _>(&s)).__locus_receiver().__logic_room());
        takes_nat((&receiver::<N<'r', N<'o', N<'o', N<'m', ()>>>>, _>(&b)).__locus_receiver().__logic_room());
        // s.len(): not on the runtime type, so on its model
        takes_nat((&receiver::<N<'l', N<'e', N<'n', ()>>>, _>(&s)).__locus_receiver().__logic_len());
        takes_nat((&receiver::<N<'l', N<'e', N<'n', ()>>>, _>(&v)).__locus_receiver().__logic_len());
        takes_nat((&receiver::<N<'l', N<'e', N<'n', ()>>>, _>(&t)).__locus_receiver().__logic_len());
        // p.twice(): on a runtime type with no model
        takes_nat((&receiver::<N<'t', N<'w', N<'i', N<'c', N<'e', ()>>>>>, _>(&p)).__locus_receiver().__logic_twice());
        #[cfg(nomodel)] takes_nat((&receiver::<N<'l', N<'e', N<'n', ()>>>, _>(&p)).__locus_receiver().__logic_len());
        #[cfg(nomethod)] takes_nat((&receiver::<N<'t', N<'o', N<'p', ()>>>, _>(&s)).__locus_receiver().__logic_top());
    };
}
fn main() { println!("q3 ok"); }
