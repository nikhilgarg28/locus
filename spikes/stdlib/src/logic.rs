//! Executable oracles for logical Seq/Map. Their Locus definitions are in locus/.
//! Observe is instrumentation; Locus Snapshot remains compiler-owned.
use std::fmt::Debug;

pub trait Observe {
    type Model: Clone + Eq + Debug;
    fn observe(&self) -> Self::Model;
}
macro_rules! scalars { ($($t:ty),*) => { $(impl Observe for $t {
    type Model = Self; fn observe(&self)->Self { *self }
})* }; }
scalars!(u8, u16, u32, u64, usize, i32, i64, bool, ());
impl Observe for String {
    type Model = String;
    fn observe(&self) -> String {
        self.clone()
    }
}
impl<A: Observe, B: Observe> Observe for (A, B) {
    type Model = (A::Model, B::Model);
    fn observe(&self) -> Self::Model {
        (self.0.observe(), self.1.observe())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seq<A>(pub Vec<A>);
impl<A> Seq<A> {
    pub fn empty() -> Self {
        Self(Vec::new())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, i: usize) -> Option<&A> {
        self.0.get(i)
    }
    pub fn push(&mut self, x: A) {
        self.0.push(x);
    }
    pub fn update(&mut self, i: usize, x: A) {
        if i < self.len() {
            self.0[i] = x;
        }
    }
}

/// Finite partial map with unique keys. This executable oracle does NOT require
/// ordering or hashing. Equality is extensional and ignores insertion order.
#[derive(Clone, Debug)]
pub struct Map<K, V>(Vec<(K, V)>);
impl<K: Eq, V> Map<K, V> {
    pub fn empty() -> Self {
        Self(Vec::new())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        self.0.iter().find(|(x, _)| x == k).map(|(_, v)| v)
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if let Some((_, old)) = self.0.iter_mut().find(|(x, _)| x == &k) {
            return Some(std::mem::replace(old, v));
        }
        self.0.push((k, v));
        None
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let i = self.0.iter().position(|(x, _)| x == k)?;
        Some(self.0.remove(i).1)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(k, v)| (k, v))
    }
}
impl<K: Eq, V: PartialEq> PartialEq for Map<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}
impl<K: Eq, V: Eq> Eq for Map<K, V> {}

/// These runtime traits stand in for checked methods + separately proved laws.
/// They cannot enforce laws in Rust. Tests supply adversarial implementations.
pub trait LawfulEq: Observe {
    fn eq_checked(&self, other: &Self) -> bool;
}
pub trait TotalOrd: LawfulEq {
    fn cmp_checked(&self, other: &Self) -> std::cmp::Ordering;
}
pub trait HashEq: LawfulEq {
    fn hash_checked(&self) -> u64;
}
macro_rules! keys { ($($t:ty),*) => { $(
impl LawfulEq for $t { fn eq_checked(&self,o:&Self)->bool {self==o} }
impl TotalOrd for $t { fn cmp_checked(&self,o:&Self)->std::cmp::Ordering {self.cmp(o)} }
impl HashEq for $t { fn hash_checked(&self)->u64 { (*self as u64).wrapping_mul(0x9e3779b97f4a7c15) } }
)* }; }
keys!(u32, u64, usize, i32, i64);

/// Fail-closed shadow instrumentation: None means there is no valid observation.
/// This is not the current always-observable Locus collection API.
pub struct Shadow<M> {
    #[cfg(feature = "audit")]
    value: Option<M>,
    // The erased carrier must not acquire M's ownership/auto-trait requirements.
    // In audit mode the real Option<M> still has those instrumentation costs.
    _type: std::marker::PhantomData<fn() -> M>,
}
impl<M> Shadow<M> {
    pub fn new(f: impl FnOnce() -> M) -> Self {
        #[cfg(feature = "audit")]
        let value = Some(f());
        #[cfg(not(feature = "audit"))]
        let _ = f;
        Self {
            #[cfg(feature = "audit")]
            value,
            _type: std::marker::PhantomData,
        }
    }
    pub fn invalidate(&mut self) {
        #[cfg(feature = "audit")]
        {
            self.value = None;
        }
    }
    pub fn take(&mut self) -> Option<M> {
        #[cfg(feature = "audit")]
        {
            self.value.take()
        }
        #[cfg(not(feature = "audit"))]
        {
            None
        }
    }
    pub fn replace(&mut self, value: Option<M>) {
        #[cfg(feature = "audit")]
        {
            self.value = value;
        }
        #[cfg(not(feature = "audit"))]
        let _ = value;
    }
    pub fn refresh(&mut self, f: impl FnOnce() -> M) {
        #[cfg(feature = "audit")]
        {
            self.value = Some(f());
        }
        #[cfg(not(feature = "audit"))]
        let _ = f;
    }
    pub fn get(&self) -> Option<&M> {
        #[cfg(feature = "audit")]
        {
            self.value.as_ref()
        }
        #[cfg(not(feature = "audit"))]
        {
            None
        }
    }
}
