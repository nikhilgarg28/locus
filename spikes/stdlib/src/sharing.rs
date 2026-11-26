//! Shared ownership requires an additional trusted lifetime/count protocol.
//! Unique Region alone is insufficient. These native implementations expose that
//! extra unsafe boundary rather than disguising std Rc/Arc as permission proofs.
use crate::{
    logic::{Observe, Shadow},
    permission::PBox,
};
use std::{
    cell::Cell,
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering, fence},
};

struct RcInner<T> {
    count: Cell<usize>,
    data: PBox<T>,
}
pub struct PRc<T> {
    ptr: NonNull<RcInner<T>>,
    _local: PhantomData<std::rc::Rc<()>>,
}
impl<T> PRc<T> {
    pub fn new(x: T) -> Self {
        let p = Box::into_raw(Box::new(RcInner {
            count: Cell::new(1),
            data: PBox::new(x),
        }));
        Self {
            ptr: NonNull::new(p).unwrap(),
            _local: PhantomData,
        }
    }
    fn inner(&self) -> &RcInner<T> {
        unsafe { self.ptr.as_ref() }
    }
    pub fn strong_count(&self) -> usize {
        self.inner().count.get()
    }
    pub fn get(&self) -> &T {
        self.inner().data.get()
    }
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.strong_count() != 1 {
            return None;
        }
        Some(unsafe { self.ptr.as_mut() }.data.get_mut())
    }
}
impl<T> Clone for PRc<T> {
    fn clone(&self) -> Self {
        let n = self
            .strong_count()
            .checked_add(1)
            .expect("Rc count overflow");
        self.inner().count.set(n);
        Self {
            ptr: self.ptr,
            _local: PhantomData,
        }
    }
}
impl<T> Drop for PRc<T> {
    fn drop(&mut self) {
        let n = self.strong_count();
        assert!(n > 0);
        if n == 1 {
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        } else {
            self.inner().count.set(n - 1);
        }
    }
}
impl<T> std::ops::Deref for PRc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get()
    }
}

struct ArcInner<T> {
    count: AtomicUsize,
    data: PBox<T>,
}
pub struct PArc<T> {
    ptr: NonNull<ArcInner<T>>,
    _owns: PhantomData<T>,
}
unsafe impl<T: Send + Sync> Send for PArc<T> {}
unsafe impl<T: Send + Sync> Sync for PArc<T> {}
impl<T> PArc<T> {
    pub fn new(x: T) -> Self {
        let p = Box::into_raw(Box::new(ArcInner {
            count: AtomicUsize::new(1),
            data: PBox::new(x),
        }));
        Self {
            ptr: NonNull::new(p).unwrap(),
            _owns: PhantomData,
        }
    }
    fn inner(&self) -> &ArcInner<T> {
        unsafe { self.ptr.as_ref() }
    }
    pub fn get(&self) -> &T {
        self.inner().data.get()
    }
    pub fn strong_count(&self) -> usize {
        self.inner().count.load(Ordering::Acquire)
    }
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.strong_count() != 1 {
            return None;
        }
        Some(unsafe { self.ptr.as_mut() }.data.get_mut())
    }
}
impl<T> Clone for PArc<T> {
    fn clone(&self) -> Self {
        let old = self.inner().count.fetch_add(1, Ordering::Relaxed);
        // Abort before overflow can produce an aliased allocation with count one.
        if old >= isize::MAX as usize {
            std::process::abort();
        }
        Self {
            ptr: self.ptr,
            _owns: PhantomData,
        }
    }
}
impl<T> Drop for PArc<T> {
    fn drop(&mut self) {
        if self.inner().count.fetch_sub(1, Ordering::Release) == 1 {
            fence(Ordering::Acquire);
            unsafe {
                drop(Box::from_raw(self.ptr.as_ptr()));
            }
        }
    }
}
impl<T> std::ops::Deref for PArc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.get()
    }
}

// The wrapped forms retain native sharing. Shadow copies observe only the stable
// snapshot footprint; observing interior payloads here would be an invalid Observe.
pub struct WRc<T: Observe> {
    raw: std::rc::Rc<T>,
    shadow: std::rc::Rc<Shadow<T::Model>>,
}
impl<T: Observe> WRc<T> {
    pub fn new(x: T) -> Self {
        Self::from_std(std::rc::Rc::new(x))
    }
    pub fn from_std(raw: std::rc::Rc<T>) -> Self {
        let shadow = std::rc::Rc::new(Shadow::new(|| raw.observe()));
        Self { raw, shadow }
    }
    pub fn get(&self) -> &T {
        &self.raw
    }
    pub fn snapshot(&self) -> Option<&T::Model> {
        self.shadow.get()
    }
    pub fn to_std(&self) -> std::rc::Rc<T> {
        self.raw.clone()
    }
    pub fn strong_count(&self) -> usize {
        std::rc::Rc::strong_count(&self.raw)
    }
}
impl<T: Observe> Clone for WRc<T> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            shadow: self.shadow.clone(),
        }
    }
}
pub struct WArc<T: Observe> {
    raw: std::sync::Arc<T>,
    shadow: std::sync::Arc<Shadow<T::Model>>,
}
impl<T: Observe> WArc<T> {
    pub fn new(x: T) -> Self {
        Self::from_std(std::sync::Arc::new(x))
    }
    pub fn from_std(raw: std::sync::Arc<T>) -> Self {
        let shadow = std::sync::Arc::new(Shadow::new(|| raw.observe()));
        Self { raw, shadow }
    }
    pub fn get(&self) -> &T {
        &self.raw
    }
    pub fn snapshot(&self) -> Option<&T::Model> {
        self.shadow.get()
    }
    pub fn to_std(&self) -> std::sync::Arc<T> {
        self.raw.clone()
    }
    pub fn strong_count(&self) -> usize {
        std::sync::Arc::strong_count(&self.raw)
    }
}
impl<T: Observe> Clone for WArc<T> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            shadow: self.shadow.clone(),
        }
    }
}
