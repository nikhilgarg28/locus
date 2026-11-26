//! Trusted executable permission kernel. Dynamic checks substitute for proof
//! arguments; this instrumented capability is NOT a zero-sized erased token.
//! A pointer alone gives no access. Region owns allocation and initialization.
use crate::logic::{Observe, Seq};
use std::{
    alloc::{self, Layout},
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(1);

pub struct Ptr<T> {
    raw: NonNull<T>,
    id: u64,
}
impl<T> Copy for Ptr<T> {}
impl<T> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

pub struct Region<T> {
    ptr: Ptr<T>,
    init: Vec<bool>,
    _owns: PhantomData<T>,
}
// Region owns T. Raw addresses alone are never used without its borrow.
unsafe impl<T: Send> Send for Region<T> {}
unsafe impl<T: Sync> Sync for Region<T> {}
impl<T> Region<T> {
    pub fn allocate(cap: usize) -> Self {
        let layout = Layout::array::<T>(cap).expect("allocation layout overflow");
        // Allocate metadata first: a metadata allocation failure leaks no block.
        let init = vec![false; cap];
        let id = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |x| x.checked_add(1))
            .expect("allocation identity exhausted");
        let raw = if layout.size() == 0 {
            NonNull::dangling()
        } else {
            NonNull::new(unsafe { alloc::alloc(layout) }.cast::<T>())
                .unwrap_or_else(|| alloc::handle_alloc_error(layout))
        };
        Self {
            ptr: Ptr { raw, id },
            init,
            _owns: PhantomData,
        }
    }
    pub fn pointer(&self) -> Ptr<T> {
        self.ptr
    }
    pub fn capacity(&self) -> usize {
        self.init.len()
    }
    pub fn initialized(&self, i: usize) -> bool {
        self.init.get(i).copied().unwrap_or(false)
    }
    fn check(&self, p: Ptr<T>, i: usize, expected: bool) {
        assert!(
            p.id == self.ptr.id && p.raw == self.ptr.raw,
            "wrong allocation permission"
        );
        assert!(i < self.capacity(), "outside permitted region");
        assert_eq!(self.init[i], expected, "initialization permission mismatch");
    }
    pub fn write(&mut self, p: Ptr<T>, i: usize, value: T) {
        self.check(p, i, false);
        unsafe { p.raw.as_ptr().add(i).write(value) };
        self.init[i] = true;
    }
    pub fn take(&mut self, p: Ptr<T>, i: usize) -> T {
        self.check(p, i, true);
        self.init[i] = false;
        unsafe { p.raw.as_ptr().add(i).read() }
    }
    pub fn read(&self, p: Ptr<T>, i: usize) -> &T {
        self.check(p, i, true);
        unsafe { &*p.raw.as_ptr().add(i) }
    }
    pub fn borrow_mut(&mut self, p: Ptr<T>, i: usize) -> &mut T {
        self.check(p, i, true);
        unsafe { &mut *p.raw.as_ptr().add(i) }
    }
    pub fn slice(&self, n: usize) -> &[T] {
        assert!(n <= self.capacity() && self.init[..n].iter().all(|b| *b));
        unsafe { std::slice::from_raw_parts(self.ptr.raw.as_ptr(), n) }
    }
    pub fn slice_mut(&mut self, n: usize) -> &mut [T] {
        assert!(n <= self.capacity() && self.init[..n].iter().all(|b| *b));
        unsafe { std::slice::from_raw_parts_mut(self.ptr.raw.as_ptr(), n) }
    }
    pub fn snapshot(&self) -> Seq<Option<T::Model>>
    where
        T: Observe,
    {
        Seq((0..self.capacity())
            .map(|i| self.init[i].then(|| self.read(self.ptr, i).observe()))
            .collect())
    }
}
impl<T> Drop for Region<T> {
    fn drop(&mut self) {
        // Continue destroying remaining initialized slots if one destructor
        // unwinds. A second destructor panic during unwind aborts as in Rust.
        struct Cleanup<'a, T> {
            r: &'a mut Region<T>,
            next: usize,
        }
        impl<T> Cleanup<'_, T> {
            fn remaining(&mut self) {
                while self.next < self.r.capacity() {
                    let i = self.next;
                    self.next += 1;
                    if self.r.init[i] {
                        let x = self.r.take(self.r.ptr, i);
                        drop(x);
                    }
                }
            }
        }
        impl<T> Drop for Cleanup<'_, T> {
            fn drop(&mut self) {
                self.remaining();
                let layout = Layout::array::<T>(self.r.capacity()).unwrap();
                if layout.size() != 0 {
                    unsafe { alloc::dealloc(self.r.ptr.raw.as_ptr().cast(), layout) }
                }
            }
        }
        let mut cleanup = Cleanup { r: self, next: 0 };
        cleanup.remaining();
    }
}

pub struct PBox<T> {
    region: Region<T>,
}
impl<T> PBox<T> {
    pub fn new(x: T) -> Self {
        let mut region = Region::allocate(1);
        region.write(region.pointer(), 0, x);
        Self { region }
    }
    pub fn into_inner(mut self) -> T {
        self.region.take(self.region.pointer(), 0)
    }
    pub fn get(&self) -> &T {
        self.region.read(self.region.pointer(), 0)
    }
    pub fn get_mut(&mut self) -> &mut T {
        self.region.borrow_mut(self.region.pointer(), 0)
    }
}
impl<T> std::ops::Deref for PBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get()
    }
}
impl<T> std::ops::DerefMut for PBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.get_mut()
    }
}
impl<T: Observe> Observe for PBox<T> {
    type Model = T::Model;
    fn observe(&self) -> Self::Model {
        self.get().observe()
    }
}
