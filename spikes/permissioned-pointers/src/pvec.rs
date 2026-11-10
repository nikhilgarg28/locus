//! `Vec`, with no `unsafe` code.
#![forbid(unsafe_code)]

use crate::perm::{self, Perm, Ptr};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

/// Invariant, which in Locus would be a proof field: `perm` is for `ptr` and
/// has `cap` slots, `len <= cap`, slots `0..len` are initialized, and slots
/// `len..cap` are not.
pub struct PVec<T> {
    ptr: Ptr<T>,
    cap: usize,
    len: usize,
    perm: Perm<T>,
}

impl<T> PVec<T> {
    pub fn new() -> Self {
        let (ptr, perm) = perm::alloc::<T>(0);
        PVec {
            ptr,
            cap: 0,
            len: 0,
            perm,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, v: T) {
        if self.len == self.cap {
            self.grow();
        }
        perm::write(self.ptr, &mut self.perm, self.len, v);
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(perm::take(self.ptr, &mut self.perm, self.len))
    }

    /// Removes element `i`, moving the last element into its place.
    pub fn swap_remove(&mut self, i: usize) -> T {
        assert!(i < self.len, "index out of bounds");
        let last = self.len - 1;
        let v = perm::take(self.ptr, &mut self.perm, i);
        if i != last {
            let moved = perm::take(self.ptr, &mut self.perm, last);
            perm::write(self.ptr, &mut self.perm, i, moved);
        }
        self.len = last;
        v
    }

    pub fn as_slice(&self) -> &[T] {
        perm::slice(self.ptr, &self.perm, self.len)
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        perm::slice_mut(self.ptr, &mut self.perm, self.len)
    }

    fn grow(&mut self) {
        let new_cap = if self.cap == 0 {
            4
        } else {
            self.cap.checked_mul(2).expect("capacity overflow")
        };
        let (new_ptr, mut new_perm) = perm::alloc::<T>(new_cap);
        perm::move_slots(self.ptr, &mut self.perm, new_ptr, &mut new_perm, self.len);
        perm::free_in_place(self.ptr, &mut self.perm, self.cap);
        self.ptr = new_ptr;
        self.perm = new_perm;
        self.cap = new_cap;
    }
}

impl<T> Default for PVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Deref for PVec<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> DerefMut for PVec<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> Drop for PVec<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        perm::free_in_place(self.ptr, &mut self.perm, self.cap);
    }
}

/// An owning iterator. Its invariant differs from the vector's: slots
/// `next..len` are initialized, and the others are not.
pub struct IntoIter<T> {
    ptr: Ptr<T>,
    cap: usize,
    next: usize,
    len: usize,
    perm: Perm<T>,
}

impl<T> IntoIterator for PVec<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    /// A type with a destructor cannot be taken apart, so the destructor is
    /// kept from running and the permission is moved out through `&mut`,
    /// leaving a permission for nothing in its place.
    fn into_iter(self) -> IntoIter<T> {
        let mut this = ManuallyDrop::new(self);
        IntoIter {
            ptr: this.ptr,
            cap: this.cap,
            next: 0,
            len: this.len,
            perm: std::mem::replace(&mut this.perm, Perm::empty()),
        }
    }
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.next == self.len {
            return None;
        }
        let v = perm::take(self.ptr, &mut self.perm, self.next);
        self.next += 1;
        Some(v)
    }
}

impl<T> Drop for IntoIter<T> {
    fn drop(&mut self) {
        while self.next().is_some() {}
        perm::free_in_place(self.ptr, &mut self.perm, self.cap);
    }
}
