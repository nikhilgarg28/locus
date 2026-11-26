//! Interior access is a protocol boundary. Root snapshots are Unit; active
//! RefCell/Mutex guards can observe their readable protected values.
use crate::{
    logic::{Observe, Shadow},
    permission::PBox,
};
use std::{
    cell::{Cell, Ref, RefCell, RefMut, UnsafeCell},
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
    sync::{LockResult, Mutex, MutexGuard, PoisonError},
};

pub struct PCell<T> {
    data: UnsafeCell<PBox<T>>,
}
impl<T> PCell<T> {
    pub fn new(x: T) -> Self {
        Self {
            data: UnsafeCell::new(PBox::new(x)),
        }
    }
    pub fn replace(&self, x: T) -> T {
        unsafe { std::mem::replace((&mut *self.data.get()).get_mut(), x) }
    }
    pub fn set(&self, x: T) {
        drop(self.replace(x));
    }
    pub fn into_inner(self) -> T {
        self.data.into_inner().into_inner()
    }
    pub fn get(&self) -> T
    where
        T: Copy,
    {
        unsafe { *(&*self.data.get()).get() }
    }
}
impl<T> Observe for PCell<T> {
    type Model = ();
    fn observe(&self) {}
}

pub struct WCell<T: Observe + Copy> {
    raw: Cell<T>,
    shadow: RefCell<Shadow<T::Model>>,
}
impl<T: Observe + Copy> WCell<T> {
    pub fn new(x: T) -> Self {
        Self {
            raw: Cell::new(x),
            shadow: RefCell::new(Shadow::new(|| x.observe())),
        }
    }
    pub fn get(&self) -> T {
        let x = self.raw.get();
        if let Some(m) = self.shadow.borrow().get() {
            assert_eq!(m, &x.observe());
        }
        x
    }
    pub fn replace(&self, x: T) -> T {
        self.shadow.borrow_mut().invalidate();
        let old = self.raw.replace(x);
        self.shadow.borrow_mut().refresh(|| x.observe());
        old
    }
    pub fn set(&self, x: T) {
        self.replace(x);
    }
    pub fn into_inner(self) -> T {
        self.raw.into_inner()
    }
}
impl<T: Observe + Copy> Observe for WCell<T> {
    type Model = ();
    fn observe(&self) {}
}

pub struct PRefCell<T> {
    data: UnsafeCell<PBox<T>>,
    borrows: Cell<isize>,
}
impl<T> PRefCell<T> {
    pub fn new(x: T) -> Self {
        Self {
            data: UnsafeCell::new(PBox::new(x)),
            borrows: Cell::new(0),
        }
    }
    pub fn try_borrow(&self) -> Option<PRef<'_, T>> {
        let n = self.borrows.get();
        if n < 0 || n == isize::MAX {
            return None;
        }
        self.borrows.set(n + 1);
        Some(PRef { owner: self })
    }
    pub fn try_borrow_mut(&self) -> Option<PRefMut<'_, T>> {
        if self.borrows.get() != 0 {
            return None;
        }
        self.borrows.set(-1);
        Some(PRefMut { owner: self })
    }
    pub fn into_inner(self) -> T {
        self.data.into_inner().into_inner()
    }
}
pub struct PRef<'a, T> {
    owner: &'a PRefCell<T>,
}
impl<T> Deref for PRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { (&*self.owner.data.get()).get() }
    }
}
impl<T> Drop for PRef<'_, T> {
    fn drop(&mut self) {
        self.owner.borrows.set(self.owner.borrows.get() - 1);
    }
}
pub struct PRefMut<'a, T> {
    owner: &'a PRefCell<T>,
}
impl<T> Deref for PRefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { (&*self.owner.data.get()).get() }
    }
}
impl<T> DerefMut for PRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { (&mut *self.owner.data.get()).get_mut() }
    }
}
impl<T> Drop for PRefMut<'_, T> {
    fn drop(&mut self) {
        self.owner.borrows.set(0);
    }
}
impl<T> Observe for PRefCell<T> {
    type Model = ();
    fn observe(&self) {}
}

struct Tracked<T: Observe> {
    value: T,
    shadow: Shadow<T::Model>,
}
impl<T: Observe> Tracked<T> {
    fn new(value: T) -> Self {
        let shadow = Shadow::new(|| value.observe());
        Self { value, shadow }
    }
}
pub struct WRefCell<T: Observe> {
    raw: RefCell<Tracked<T>>,
}
impl<T: Observe> WRefCell<T> {
    pub fn new(x: T) -> Self {
        Self {
            raw: RefCell::new(Tracked::new(x)),
        }
    }
    pub fn try_borrow(&self) -> Option<WRef<'_, T>> {
        self.raw.try_borrow().ok().map(|guard| WRef { guard })
    }
    pub fn try_borrow_mut(&self) -> Option<WRefMut<'_, T>> {
        let mut guard = self.raw.try_borrow_mut().ok()?;
        guard.shadow.invalidate();
        Some(WRefMut { guard })
    }
    pub fn into_inner(self) -> T {
        self.raw.into_inner().value
    }
}
pub struct WRef<'a, T: Observe> {
    guard: Ref<'a, Tracked<T>>,
}
impl<T: Observe> Deref for WRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard.value
    }
}
impl<T: Observe> WRef<'_, T> {
    pub fn model(&self) -> Option<&T::Model> {
        self.guard.shadow.get()
    }
}
pub struct WRefMut<'a, T: Observe> {
    guard: RefMut<'a, Tracked<T>>,
}
impl<T: Observe> Deref for WRefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard.value
    }
}
impl<T: Observe> DerefMut for WRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard.value
    }
}
impl<T: Observe> Drop for WRefMut<'_, T> {
    fn drop(&mut self) {
        let t = &mut *self.guard;
        t.shadow.refresh(|| t.value.observe());
    }
}
impl<T: Observe> Observe for WRefCell<T> {
    type Model = ();
    fn observe(&self) {}
}

/// Native spin mutex: mutual exclusion + acquire/release, no fairness promise.
pub struct PMutex<T> {
    locked: AtomicBool,
    poisoned: AtomicBool,
    data: UnsafeCell<PBox<T>>,
}
unsafe impl<T: Send> Send for PMutex<T> {}
unsafe impl<T: Send> Sync for PMutex<T> {}
impl<T> PMutex<T> {
    pub fn new(x: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            poisoned: AtomicBool::new(false),
            data: UnsafeCell::new(PBox::new(x)),
        }
    }
    pub fn try_lock(&self) -> Option<LockResult<PGuard<'_, T>>> {
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }
        let guard = PGuard {
            owner: self,
            _not_send: std::marker::PhantomData,
        };
        Some(if self.poisoned.load(Ordering::Relaxed) {
            Err(PoisonError::new(guard))
        } else {
            Ok(guard)
        })
    }
    pub fn lock(&self) -> LockResult<PGuard<'_, T>> {
        loop {
            if let Some(g) = self.try_lock() {
                return g;
            }
            std::thread::yield_now();
        }
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed)
    }
}
pub struct PGuard<'a, T> {
    owner: &'a PMutex<T>,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl<T> Deref for PGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { (&*self.owner.data.get()).get() }
    }
}
impl<T> DerefMut for PGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { (&mut *self.owner.data.get()).get_mut() }
    }
}
impl<T> Drop for PGuard<'_, T> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.owner.poisoned.store(true, Ordering::Relaxed);
        }
        self.owner.locked.store(false, Ordering::Release);
    }
}
impl<T> Observe for PMutex<T> {
    type Model = ();
    fn observe(&self) {}
}

pub struct WMutex<T: Observe> {
    raw: Mutex<Tracked<T>>,
}
impl<T: Observe> WMutex<T> {
    pub fn new(x: T) -> Self {
        Self {
            raw: Mutex::new(Tracked::new(x)),
        }
    }
    pub fn lock(&self) -> LockResult<WGuard<'_, T>> {
        match self.raw.lock() {
            Ok(mut guard) => {
                guard.shadow.invalidate();
                Ok(WGuard { guard })
            }
            Err(e) => {
                let mut guard = e.into_inner();
                guard.shadow.invalidate();
                Err(PoisonError::new(WGuard { guard }))
            }
        }
    }
    pub fn try_lock(&self) -> bool {
        self.raw.try_lock().is_ok()
    }
    pub fn is_poisoned(&self) -> bool {
        self.raw.is_poisoned()
    }
}
pub struct WGuard<'a, T: Observe> {
    guard: MutexGuard<'a, Tracked<T>>,
}
impl<T: Observe> Deref for WGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard.value
    }
}
impl<T: Observe> DerefMut for WGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard.value
    }
}
impl<T: Observe> Drop for WGuard<'_, T> {
    fn drop(&mut self) {
        let t = &mut *self.guard;
        t.shadow.refresh(|| t.value.observe());
    }
}
impl<T: Observe> Observe for WMutex<T> {
    type Model = ();
    fn observe(&self) {}
}
