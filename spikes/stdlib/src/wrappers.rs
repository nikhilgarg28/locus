//! Native Rust storage with independent, erased-in-Locus shadow observations.
//! Ordinary methods use incremental models; explicit native handoff rebuilds.
#![forbid(unsafe_code)]
use crate::logic::{HashEq, Map, Observe, Seq, Shadow, TotalOrd};
use std::{
    collections::{BTreeMap, HashMap},
    hash::Hash,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

pub struct WVec<T: Observe> {
    raw: Vec<T>,
    shadow: Shadow<Seq<T::Model>>,
}
impl<T: Observe> Default for WVec<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: Observe> WVec<T> {
    pub fn new() -> Self {
        Self {
            raw: Vec::new(),
            shadow: Shadow::new(Seq::empty),
        }
    }
    pub fn from_std(raw: Vec<T>) -> Self {
        let shadow = Shadow::new(|| Seq(raw.iter().map(Observe::observe).collect()));
        Self { raw, shadow }
    }
    pub fn into_std(self) -> Vec<T> {
        self.raw
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.raw.capacity()
    }
    pub fn snapshot(&self) -> Option<&Seq<T::Model>> {
        self.shadow.get()
    }
    pub fn get(&self, i: usize) -> Option<&T> {
        self.raw.get(i)
    }
    pub fn as_slice(&self) -> &[T] {
        &self.raw
    }
    pub fn refresh(&mut self) {
        self.shadow.invalidate();
        self.shadow
            .refresh(|| Seq(self.raw.iter().map(Observe::observe).collect()));
    }
    pub fn push(&mut self, x: T) {
        let mut model = self.shadow.take();
        if let Some(m) = &mut model {
            m.push(x.observe());
        }
        self.raw.push(x);
        self.shadow.replace(model);
    }
    pub fn pop(&mut self) -> Option<T> {
        let mut model = self.shadow.take();
        let out = self.raw.pop();
        if let Some(m) = &mut model {
            m.0.pop();
        }
        self.shadow.replace(model);
        out
    }
    pub fn set(&mut self, i: usize, x: T) -> T {
        assert!(i < self.len());
        let mut model = self.shadow.take();
        if let Some(m) = &mut model {
            m.update(i, x.observe());
        }
        let old = std::mem::replace(&mut self.raw[i], x);
        self.shadow.replace(model);
        old
    }
    pub fn insert(&mut self, i: usize, x: T) {
        assert!(i <= self.len());
        let mut model = self.shadow.take();
        if let Some(m) = &mut model {
            m.0.insert(i, x.observe());
        }
        self.raw.insert(i, x);
        self.shadow.replace(model);
    }
    pub fn remove(&mut self, i: usize) -> T {
        assert!(i < self.len());
        let mut model = self.shadow.take();
        let old = self.raw.remove(i);
        if let Some(m) = &mut model {
            m.0.remove(i);
        }
        self.shadow.replace(model);
        old
    }
    /// The callback cannot return a reference tied to its temporary argument.
    /// On unwind, reobserve actual native state before resuming the panic.
    pub fn with_native<R>(&mut self, f: impl FnOnce(&mut Vec<T>) -> R) -> R {
        self.shadow.invalidate();
        let result = catch_unwind(AssertUnwindSafe(|| f(&mut self.raw)));
        self.refresh();
        match result {
            Ok(x) => x,
            Err(p) => resume_unwind(p),
        }
    }
    /// An experimental guard: forgetting it invalidates observation, never leaves
    /// a stale model labelled valid. Current Locus needs its loan-end protocol
    /// to offer an always-observable Seq after the loan instead.
    pub fn loan(&mut self) -> VecLoan<'_, T> {
        self.shadow.invalidate();
        VecLoan { owner: self }
    }
    pub fn validate(&self) {
        if let Some(m) = self.snapshot() {
            assert_eq!(m, &Seq(self.raw.iter().map(Observe::observe).collect()));
        }
    }
}
pub struct VecLoan<'a, T: Observe> {
    owner: &'a mut WVec<T>,
}
impl<T: Observe> std::ops::Deref for VecLoan<'_, T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.owner.raw
    }
}
impl<T: Observe> std::ops::DerefMut for VecLoan<'_, T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.owner.raw
    }
}
impl<T: Observe> Drop for VecLoan<'_, T> {
    fn drop(&mut self) {
        self.owner.refresh();
    }
}

pub struct WBox<T: Observe> {
    raw: Box<T>,
    shadow: Shadow<T::Model>,
}
impl<T: Observe> WBox<T> {
    pub fn new(x: T) -> Self {
        Self::from_std(Box::new(x))
    }
    pub fn from_std(raw: Box<T>) -> Self {
        let shadow = Shadow::new(|| raw.observe());
        Self { raw, shadow }
    }
    pub fn into_std(self) -> Box<T> {
        self.raw
    }
    pub fn into_inner(self) -> T {
        *self.raw
    }
    pub fn get(&self) -> &T {
        &self.raw
    }
    pub fn snapshot(&self) -> Option<&T::Model> {
        self.shadow.get()
    }
    pub fn with_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        self.shadow.invalidate();
        let result = catch_unwind(AssertUnwindSafe(|| f(&mut self.raw)));
        self.shadow.refresh(|| self.raw.observe());
        match result {
            Ok(x) => x,
            Err(p) => resume_unwind(p),
        }
    }
}

pub struct WBTreeMap<K: TotalOrd + Ord, V: Observe> {
    raw: BTreeMap<K, V>,
    shadow: Shadow<Map<K::Model, V::Model>>,
}
impl<K: TotalOrd + Ord, V: Observe> Default for WBTreeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: TotalOrd + Ord, V: Observe> WBTreeMap<K, V> {
    pub fn new() -> Self {
        Self {
            raw: BTreeMap::new(),
            shadow: Shadow::new(Map::empty),
        }
    }
    pub fn from_std(raw: BTreeMap<K, V>) -> Self {
        let mut out = Self {
            raw,
            shadow: Shadow::new(Map::empty),
        };
        out.refresh();
        out
    }
    pub fn into_std(self) -> BTreeMap<K, V> {
        self.raw
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
    pub fn snapshot(&self) -> Option<&Map<K::Model, V::Model>> {
        self.shadow.get()
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        self.raw.get(k)
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        let mut m = self.shadow.take();
        if let Some(m) = &mut m {
            m.insert(k.observe(), v.observe());
        }
        let old = self.raw.insert(k, v);
        self.shadow.replace(m);
        old
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let mut m = self.shadow.take();
        if let Some(m) = &mut m {
            m.remove(&k.observe());
        }
        let old = self.raw.remove(k);
        self.shadow.replace(m);
        old
    }
    fn actual(&self) -> Map<K::Model, V::Model> {
        let mut m = Map::empty();
        for (k, v) in &self.raw {
            assert!(
                m.insert(k.observe(), v.observe()).is_none(),
                "logical key collision"
            );
        }
        m
    }
    pub fn refresh(&mut self) {
        self.shadow.invalidate();
        #[cfg(feature = "audit")]
        {
            let m = self.actual();
            self.shadow.replace(Some(m));
        }
    }
    pub fn with_native<R>(&mut self, f: impl FnOnce(&mut BTreeMap<K, V>) -> R) -> R {
        self.shadow.invalidate();
        let r = catch_unwind(AssertUnwindSafe(|| f(&mut self.raw)));
        self.refresh();
        match r {
            Ok(x) => x,
            Err(p) => resume_unwind(p),
        }
    }
    pub fn validate(&self) {
        if let Some(m) = self.snapshot() {
            assert_eq!(m, &self.actual());
        }
        for a in self.raw.keys() {
            for b in self.raw.keys() {
                assert_eq!(a.cmp(b), a.cmp_checked(b), "Ord bridge mismatch");
            }
        }
    }
}
pub struct WHashMap<K: HashEq + Eq + Hash, V: Observe> {
    raw: HashMap<K, V>,
    shadow: Shadow<Map<K::Model, V::Model>>,
}
impl<K: HashEq + Eq + Hash, V: Observe> Default for WHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: HashEq + Eq + Hash, V: Observe> WHashMap<K, V> {
    pub fn new() -> Self {
        Self {
            raw: HashMap::new(),
            shadow: Shadow::new(Map::empty),
        }
    }
    pub fn from_std(raw: HashMap<K, V>) -> Self {
        let mut out = Self {
            raw,
            shadow: Shadow::new(Map::empty),
        };
        out.refresh();
        out
    }
    pub fn into_std(self) -> HashMap<K, V> {
        self.raw
    }
    pub fn len(&self) -> usize {
        self.raw.len()
    }
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
    pub fn snapshot(&self) -> Option<&Map<K::Model, V::Model>> {
        self.shadow.get()
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        self.raw.get(k)
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        let mut m = self.shadow.take();
        if let Some(m) = &mut m {
            m.insert(k.observe(), v.observe());
        }
        let old = self.raw.insert(k, v);
        self.shadow.replace(m);
        old
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let mut m = self.shadow.take();
        if let Some(m) = &mut m {
            m.remove(&k.observe());
        }
        let old = self.raw.remove(k);
        self.shadow.replace(m);
        old
    }
    fn actual(&self) -> Map<K::Model, V::Model> {
        let mut m = Map::empty();
        for (k, v) in &self.raw {
            assert!(
                m.insert(k.observe(), v.observe()).is_none(),
                "logical key collision"
            );
        }
        m
    }
    pub fn refresh(&mut self) {
        self.shadow.invalidate();
        #[cfg(feature = "audit")]
        {
            let m = self.actual();
            self.shadow.replace(Some(m));
        }
    }
    pub fn with_native<R>(&mut self, f: impl FnOnce(&mut HashMap<K, V>) -> R) -> R {
        self.shadow.invalidate();
        let r = catch_unwind(AssertUnwindSafe(|| f(&mut self.raw)));
        self.refresh();
        match r {
            Ok(x) => x,
            Err(p) => resume_unwind(p),
        }
    }
    pub fn validate(&self) {
        if let Some(m) = self.snapshot() {
            assert_eq!(m, &self.actual());
        }
        for a in self.raw.keys() {
            for b in self.raw.keys() {
                assert_eq!(a == b, a.eq_checked(b), "Eq bridge mismatch");
            }
        }
    }
}
