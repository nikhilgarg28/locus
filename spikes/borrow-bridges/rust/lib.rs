//! Hand-lowered Rust shapes only. No marker in this crate is a Locus proof.
#![forbid(unsafe_code)]

use std::borrow::Borrow;
use std::hash::{Hash, Hasher};

/// The proof-erased shape of an independently verified borrowing operation.
/// Deliberately does NOT imply that an existing Borrow implementation agrees.
pub trait BorrowOperation<Q: ?Sized> {
    fn borrow_verified(&self) -> &Q;
}

// Reuse Rust's identity implementation instead of generating Borrow<T> again.
impl<T: ?Sized> BorrowOperation<T> for T {
    fn borrow_verified(&self) -> &T {
        <T as Borrow<T>>::borrow(self)
    }
}

impl BorrowOperation<str> for String {
    fn borrow_verified(&self) -> &str {
        <String as Borrow<str>>::borrow(self)
    }
}

impl<T> BorrowOperation<[T]> for Vec<T> {
    fn borrow_verified(&self) -> &[T] {
        <Vec<T> as Borrow<[T]>>::borrow(self)
    }
}

/// Emulate a provider's instantiated, concrete impl. This is ordinary Rust
/// code generation, not a general Locus provider implementation or proof check.
#[macro_export]
macro_rules! provide_text_borrow {
    ($key:ty) => {
        impl std::borrow::Borrow<str> for $key {
            fn borrow(&self) -> &str {
                <$key as $crate::BorrowOperation<str>>::borrow_verified(self)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(pub String);

impl BorrowOperation<str> for Name {
    fn borrow_verified(&self) -> &str {
        &self.0
    }
}
provide_text_borrow!(Name);

/// A generic concrete wrapper can borrow its own T, including unsized T.
/// This is narrower than forwarding arbitrary Q with T: Borrow<Q>.
pub struct Owned<T: ?Sized>(pub Box<T>);
impl<T: ?Sized> Borrow<T> for Owned<T> {
    fn borrow(&self) -> &T {
        &self.0
    }
}

mod private {
    pub trait Sealed<K: ?Sized, Q: ?Sized> {}
}

/// Illustrative closed registry, protecting only the adapters listed below.
/// This does NOT solve open, cross-crate Locus certificate registration.
pub trait NativeBorrowBinding<K: ?Sized, Q: ?Sized>: private::Sealed<K, Q> {}
pub struct StdText;
pub struct Identity;
pub struct GeneratedName;

impl private::Sealed<String, str> for StdText {}
impl NativeBorrowBinding<String, str> for StdText {}
impl<T: ?Sized> private::Sealed<T, T> for Identity {}
impl<T: ?Sized> NativeBorrowBinding<T, T> for Identity {}
impl private::Sealed<Name, str> for GeneratedName {}
impl NativeBorrowBinding<Name, str> for GeneratedName {}

/// Dispatch to the actual native impl; no separate user operation is called.
pub fn native_borrow<K: ?Sized, Q: ?Sized, A>(key: &K) -> &Q
where
    K: Borrow<Q>,
    A: NativeBorrowBinding<K, Q>,
{
    <K as Borrow<Q>>::borrow(key)
}

/// Finite runtime instrumentation of primitive calls, not Locus's ghost trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Bytes(Vec<u8>),
    U8(u8),
    U32(u32),
    Usize(usize),
}

#[derive(Default)]
pub struct Recorder(pub Vec<Event>);
impl Hasher for Recorder {
    fn finish(&self) -> u64 {
        0 // Tests inspect events, never infer trace equality from this digest.
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.push(Event::Bytes(bytes.to_vec()));
    }
    fn write_u8(&mut self, n: u8) {
        self.0.push(Event::U8(n));
    }
    fn write_u32(&mut self, n: u32) {
        self.0.push(Event::U32(n));
    }
    fn write_usize(&mut self, n: usize) {
        self.0.push(Event::Usize(n));
    }
}

pub fn trace<T: Hash + ?Sized>(value: &T) -> Vec<Event> {
    let mut recorder = Recorder::default();
    value.hash(&mut recorder);
    recorder.0
}
