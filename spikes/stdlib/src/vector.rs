#![forbid(unsafe_code)]
use crate::{
    logic::{Observe, Seq},
    permission::Region,
};

/// Actual element storage is a permissioned allocation, not std::Vec.
pub struct PVec<T> {
    region: Region<T>,
    len: usize,
}
impl<T> Default for PVec<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> PVec<T> {
    pub fn new() -> Self {
        Self {
            region: Region::allocate(0),
            len: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn capacity(&self) -> usize {
        self.region.capacity()
    }
    pub fn reserve(&mut self, additional: usize) {
        let need = self.len.checked_add(additional).expect("length overflow");
        if need <= self.capacity() {
            return;
        }
        let cap = need.max(self.capacity().saturating_mul(2)).max(4);
        let mut next = Region::allocate(cap);
        for i in 0..self.len {
            let x = self.region.take(self.region.pointer(), i);
            next.write(next.pointer(), i, x);
        }
        self.region = next;
    }
    pub fn push(&mut self, x: T) {
        self.reserve(1);
        self.region.write(self.region.pointer(), self.len, x);
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(self.region.take(self.region.pointer(), self.len))
        }
    }
    pub fn get(&self, i: usize) -> Option<&T> {
        (i < self.len).then(|| self.region.read(self.region.pointer(), i))
    }
    pub fn get_mut(&mut self, i: usize) -> Option<&mut T> {
        if i < self.len {
            Some(self.region.borrow_mut(self.region.pointer(), i))
        } else {
            None
        }
    }
    pub fn insert(&mut self, i: usize, x: T) {
        assert!(i <= self.len);
        self.reserve(1);
        for j in (i..self.len).rev() {
            let v = self.region.take(self.region.pointer(), j);
            self.region.write(self.region.pointer(), j + 1, v);
        }
        self.region.write(self.region.pointer(), i, x);
        self.len += 1;
    }
    pub fn remove(&mut self, i: usize) -> T {
        assert!(i < self.len);
        let x = self.region.take(self.region.pointer(), i);
        for j in i + 1..self.len {
            let v = self.region.take(self.region.pointer(), j);
            self.region.write(self.region.pointer(), j - 1, v);
        }
        self.len -= 1;
        x
    }
    pub fn set(&mut self, i: usize, x: T) -> T {
        std::mem::replace(self.get_mut(i).expect("index"), x)
    }
    pub fn as_slice(&self) -> &[T] {
        self.region.slice(self.len)
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.region.slice_mut(self.len)
    }
    pub fn into_std(mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.region.take(self.region.pointer(), i));
        }
        self.len = 0;
        out
    }
    pub fn from_std(v: Vec<T>) -> Self {
        let mut out = Self::new();
        out.reserve(v.len());
        for x in v {
            out.push(x);
        }
        out
    }
}
impl<T: Observe> Observe for PVec<T> {
    type Model = Seq<T::Model>;
    fn observe(&self) -> Self::Model {
        Seq(self.as_slice().iter().map(Observe::observe).collect())
    }
}
impl<T> std::ops::Index<usize> for PVec<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        self.get(i).expect("index")
    }
}
impl<T> std::ops::IndexMut<usize> for PVec<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        self.get_mut(i).expect("index")
    }
}
