//! Linear-probing hash map with tombstones, stored hashes, growth and deletion.
#![forbid(unsafe_code)]
use crate::{
    logic::{HashEq, Map, Observe},
    vector::PVec,
};
enum Bucket<K, V> {
    Empty,
    Deleted,
    Full { hash: u64, key: K, value: V },
}
pub struct PHashMap<K, V> {
    buckets: PVec<Bucket<K, V>>,
    len: usize,
    used: usize,
}
impl<K: HashEq, V> Default for PHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: HashEq, V> PHashMap<K, V> {
    pub fn new() -> Self {
        Self {
            buckets: PVec::new(),
            len: 0,
            used: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    fn allocate(cap: usize) -> PVec<Bucket<K, V>> {
        let mut b = PVec::new();
        for _ in 0..cap {
            b.push(Bucket::Empty);
        }
        b
    }
    fn find(&self, k: &K, h: u64) -> Result<usize, usize> {
        let cap = self.buckets.len();
        assert!(cap > 0);
        let mut deleted = None;
        for distance in 0..cap {
            let i = ((h as usize).wrapping_add(distance)) & (cap - 1);
            match &self.buckets[i] {
                Bucket::Empty => return Err(deleted.unwrap_or(i)),
                Bucket::Deleted => {
                    if deleted.is_none() {
                        deleted = Some(i);
                    }
                }
                Bucket::Full { hash, key, .. } if *hash == h && key.eq_checked(k) => return Ok(i),
                _ => {}
            }
        }
        Err(deleted.expect("load invariant"))
    }
    fn rebuild(&mut self, cap: usize) {
        // Cached hashes make rehashing move-only: it calls no user Hash or Eq.
        let next = Self::allocate(cap);
        let mut old = std::mem::replace(&mut self.buckets, next);
        self.len = 0;
        self.used = 0;
        while let Some(b) = old.pop() {
            if let Bucket::Full { hash, key, value } = b {
                let mut i = (hash as usize) & (cap - 1);
                while !matches!(self.buckets[i], Bucket::Empty) {
                    i = (i + 1) & (cap - 1);
                }
                self.buckets[i] = Bucket::Full { hash, key, value };
                self.len += 1;
                self.used += 1;
            }
        }
    }
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let h = key.hash_checked();
        if self.buckets.is_empty() {
            self.rebuild(8);
        }
        if self.used >= self.buckets.len() / 2 {
            self.rebuild(self.buckets.len().checked_mul(2).expect("capacity"));
        }
        match self.find(&key, h) {
            Ok(i) => {
                if let Bucket::Full { value: old, .. } = &mut self.buckets[i] {
                    Some(std::mem::replace(old, value))
                } else {
                    unreachable!()
                }
            }
            Err(i) => {
                if matches!(self.buckets[i], Bucket::Empty) {
                    self.used += 1;
                }
                self.buckets[i] = Bucket::Full {
                    hash: h,
                    key,
                    value,
                };
                self.len += 1;
                None
            }
        }
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        if self.buckets.is_empty() {
            return None;
        }
        let i = self.find(key, key.hash_checked()).ok()?;
        if let Bucket::Full { value, .. } = &self.buckets[i] {
            Some(value)
        } else {
            unreachable!()
        }
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if self.buckets.is_empty() {
            return None;
        }
        let i = self.find(key, key.hash_checked()).ok()?;
        if let Bucket::Full { value, .. } = std::mem::replace(&mut self.buckets[i], Bucket::Deleted)
        {
            self.len -= 1;
            Some(value)
        } else {
            unreachable!()
        }
    }
    pub fn visit(&self, mut f: impl FnMut(&K, &V)) {
        for b in self.buckets.as_slice() {
            if let Bucket::Full { key, value, .. } = b {
                f(key, value);
            }
        }
    }
    pub fn validate(&self) {
        let mut n = 0;
        let mut used = 0;
        for (i, b) in self.buckets.as_slice().iter().enumerate() {
            match b {
                Bucket::Full { hash, key, .. } => {
                    n += 1;
                    used += 1;
                    assert_eq!(self.find(key, *hash), Ok(i));
                }
                Bucket::Deleted => used += 1,
                Bucket::Empty => {}
            }
        }
        assert_eq!((n, used), (self.len, self.used));
    }
    pub fn into_entries(mut self) -> PVec<(K, V)> {
        let mut out = PVec::new();
        while let Some(b) = self.buckets.pop() {
            if let Bucket::Full { key, value, .. } = b {
                out.push((key, value));
            }
        }
        out
    }
}
impl<K: HashEq, V: Observe> Observe for PHashMap<K, V> {
    type Model = Map<K::Model, V::Model>;
    fn observe(&self) -> Self::Model {
        let mut m = Map::empty();
        self.visit(|k, v| {
            assert!(
                m.insert(k.observe(), v.observe()).is_none(),
                "key abstraction collapsed distinct keys"
            );
        });
        m
    }
}
