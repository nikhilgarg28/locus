//! Executable instrumentation for the .lc checked signatures. Evidence is a
//! zero-sized test receipt, NOT a kernel proof. Audit checks actual before/after
//! observations. Non-audit builds omit those checks and all observation reads.
use crate::{
    btree::PBTreeMap,
    hash::PHashMap,
    logic::{HashEq, Map, Observe, Seq, TotalOrd},
    vector::PVec,
    wrappers::{WBTreeMap, WHashMap, WVec},
};
#[derive(Debug)]
pub struct Evidence(());
pub struct Inside {
    index: usize,
    len: usize,
}
pub fn inside(index: usize, len: usize) -> Option<Inside> {
    (index < len).then_some(Inside { index, len })
}

pub trait CheckedVec<T: Observe> {
    fn runtime_len(&self) -> usize;
    fn runtime_push(&mut self, x: T);
    fn runtime_pop(&mut self) -> Option<T>;
    fn runtime_get(&self, i: usize) -> Option<&T>;
    fn model_now(&self) -> Seq<T::Model>;
    fn push_checked(&mut self, x: T) -> Evidence {
        #[cfg(feature = "audit")]
        let expected = {
            let mut s = self.model_now();
            s.push(x.observe());
            s
        };
        self.runtime_push(x);
        #[cfg(feature = "audit")]
        assert_eq!(self.model_now(), expected);
        Evidence(())
    }
    fn pop_checked(&mut self) -> (Option<T>, Evidence) {
        #[cfg(feature = "audit")]
        let mut expected = self.model_now();
        let result = self.runtime_pop();
        #[cfg(feature = "audit")]
        {
            let last = expected.0.pop();
            assert_eq!(result.as_ref().map(Observe::observe), last);
            assert_eq!(self.model_now(), expected);
        }
        (result, Evidence(()))
    }
    fn get_checked(&self, i: usize, p: Inside) -> (&T, Evidence) {
        assert_eq!(
            (p.index, p.len),
            (i, self.runtime_len()),
            "stale/mismatched bound witness"
        );
        let item = self.runtime_get(i).expect("bound");
        #[cfg(feature = "audit")]
        assert_eq!(self.model_now().get(i), Some(&item.observe()));
        (item, Evidence(()))
    }
}
impl<T: Observe> CheckedVec<T> for PVec<T> {
    fn runtime_len(&self) -> usize {
        self.len()
    }
    fn runtime_push(&mut self, x: T) {
        self.push(x)
    }
    fn runtime_pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn runtime_get(&self, i: usize) -> Option<&T> {
        self.get(i)
    }
    fn model_now(&self) -> Seq<T::Model> {
        self.observe()
    }
}
impl<T: Observe> CheckedVec<T> for WVec<T> {
    fn runtime_len(&self) -> usize {
        self.len()
    }
    fn runtime_push(&mut self, x: T) {
        self.push(x)
    }
    fn runtime_pop(&mut self) -> Option<T> {
        self.pop()
    }
    fn runtime_get(&self, i: usize) -> Option<&T> {
        self.get(i)
    }
    fn model_now(&self) -> Seq<T::Model> {
        #[cfg(feature = "audit")]
        {
            self.snapshot().expect("observation invalidated").clone()
        }
        #[cfg(not(feature = "audit"))]
        {
            Seq(self.as_slice().iter().map(Observe::observe).collect())
        }
    }
}
pub trait CheckedMap<K: Observe, V: Observe> {
    fn model_now(&self) -> Map<K::Model, V::Model>;
    fn runtime_insert(&mut self, k: K, v: V) -> Option<V>;
    fn runtime_get(&self, k: &K) -> Option<&V>;
    fn runtime_remove(&mut self, k: &K) -> Option<V>;
    fn insert_checked(&mut self, k: K, v: V) -> (Option<V>, Evidence) {
        #[cfg(feature = "audit")]
        let (mut expected, key, value) = (self.model_now(), k.observe(), v.observe());
        #[cfg(feature = "audit")]
        let previous = expected.insert(key, value);
        let result = self.runtime_insert(k, v);
        #[cfg(feature = "audit")]
        {
            assert_eq!(result.as_ref().map(Observe::observe), previous);
            assert_eq!(self.model_now(), expected);
        }
        (result, Evidence(()))
    }
    fn get_checked(&self, k: &K) -> (Option<&V>, Evidence) {
        let result = self.runtime_get(k);
        #[cfg(feature = "audit")]
        assert_eq!(
            result.map(Observe::observe),
            self.model_now().get(&k.observe()).cloned()
        );
        (result, Evidence(()))
    }
    fn remove_checked(&mut self, k: &K) -> (Option<V>, Evidence) {
        #[cfg(feature = "audit")]
        let mut expected = self.model_now();
        #[cfg(feature = "audit")]
        let previous = expected.remove(&k.observe());
        let result = self.runtime_remove(k);
        #[cfg(feature = "audit")]
        {
            assert_eq!(result.as_ref().map(Observe::observe), previous);
            assert_eq!(self.model_now(), expected);
        }
        (result, Evidence(()))
    }
}
macro_rules! native_map {
    ($name:ident,$bound:path) => {
        impl<K: $bound, V: Observe> CheckedMap<K, V> for $name<K, V> {
            fn model_now(&self) -> Map<K::Model, V::Model> {
                self.observe()
            }
            fn runtime_insert(&mut self, k: K, v: V) -> Option<V> {
                self.insert(k, v)
            }
            fn runtime_get(&self, k: &K) -> Option<&V> {
                self.get(k)
            }
            fn runtime_remove(&mut self, k: &K) -> Option<V> {
                self.remove(k)
            }
        }
    };
}
native_map!(PBTreeMap, TotalOrd);
native_map!(PHashMap, HashEq);
impl<K: TotalOrd + Ord, V: Observe> CheckedMap<K, V> for WBTreeMap<K, V> {
    fn model_now(&self) -> Map<K::Model, V::Model> {
        self.snapshot().expect("audit model unavailable").clone()
    }
    fn runtime_insert(&mut self, k: K, v: V) -> Option<V> {
        self.insert(k, v)
    }
    fn runtime_get(&self, k: &K) -> Option<&V> {
        self.get(k)
    }
    fn runtime_remove(&mut self, k: &K) -> Option<V> {
        self.remove(k)
    }
}
impl<K: HashEq + Eq + std::hash::Hash, V: Observe> CheckedMap<K, V> for WHashMap<K, V> {
    fn model_now(&self) -> Map<K::Model, V::Model> {
        self.snapshot().expect("audit model unavailable").clone()
    }
    fn runtime_insert(&mut self, k: K, v: V) -> Option<V> {
        self.insert(k, v)
    }
    fn runtime_get(&self, k: &K) -> Option<&V> {
        self.get(k)
    }
    fn runtime_remove(&mut self, k: &K) -> Option<V> {
        self.remove(k)
    }
}
