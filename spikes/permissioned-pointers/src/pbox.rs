//! `Box`, with no `unsafe` code.
#![forbid(unsafe_code)]

use crate::perm::{self, Perm, Ptr};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};

/// Invariant, which in Locus would be a proof field: `perm` is for `ptr`,
/// has one slot, and the slot is initialized.
pub struct PBox<T> {
    ptr: Ptr<T>,
    perm: Perm<T>,
}

impl<T> PBox<T> {
    pub fn new(v: T) -> Self {
        let (ptr, mut perm) = perm::alloc::<T>(1);
        perm::write(ptr, &mut perm, 0, v);
        PBox { ptr, perm }
    }

    /// Takes the value and frees the block.
    ///
    /// Rust does not let a type with a destructor be taken apart, so this
    /// keeps the destructor from running and works through `&mut` instead.
    /// The invariant is broken from the `take` on, and never restored.
    pub fn into_inner(self) -> T {
        let mut this = ManuallyDrop::new(self);
        let v = perm::take(this.ptr, &mut this.perm, 0);
        perm::free_in_place(this.ptr, &mut this.perm, 1);
        v
    }
}

impl<T> Deref for PBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        perm::get(self.ptr, &self.perm, 0)
    }
}

impl<T> DerefMut for PBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        perm::get_mut(self.ptr, &mut self.perm, 0)
    }
}

impl<T> Drop for PBox<T> {
    /// `drop` has only `&mut self`, so it cannot give the permission to a
    /// function that consumes it. It leaves the invariant broken, which is
    /// sound because nothing observes the value again.
    fn drop(&mut self) {
        drop(perm::take(self.ptr, &mut self.perm, 0));
        perm::free_in_place(self.ptr, &mut self.perm, 1);
    }
}
