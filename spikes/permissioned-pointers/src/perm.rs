//! The trusted layer. Every `unsafe` block of the spike is in this file.
//!
//! A `Ptr<T>` is an address. A `Perm<T>` is the right to use the block of
//! slots at one address, and it knows which slots are initialized. In Locus
//! the permission would be a ghost value that rustc sees as a zero-sized
//! marker, and each `requires` below would be a proof that the caller gives.
//!
//! Plain Rust cannot check those proofs, so the spike has two builds:
//! - the default build, where `Perm<T>` is zero-sized and nothing is checked;
//! - `--cfg checked`, where `Perm<T>` carries the state a proof would speak
//!   of, and every primitive asserts its `requires` against it.
#![allow(unsafe_code)]

use std::alloc::{self, Layout};
use std::marker::PhantomData;
use std::ptr::NonNull;

/// An address. It is `Copy`, and holding one gives no right to the memory.
pub struct Ptr<T> {
    raw: NonNull<T>,
}

impl<T> Clone for Ptr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Ptr<T> {}

// An address owns nothing, so it may cross threads whatever `T` is, as a
// `usize` may. What a collection is allowed to do follows from its `Perm<T>`.
unsafe impl<T> Send for Ptr<T> {}
unsafe impl<T> Sync for Ptr<T> {}

/// The permission to use one block. Neither `Copy` nor `Clone`: rustc's
/// ownership and borrow rules keep it unique, with no help from Locus.
///
/// `PhantomData<T>` makes it covariant in `T`, an owner of `T` for the drop
/// check, and `Send` or `Sync` exactly when `T` is.
pub struct Perm<T> {
    _owns: PhantomData<T>,
    #[cfg(checked)]
    shadow: shadow::Shadow,
}

impl<T> Perm<T> {
    /// A permission for nothing. Any code may make one, since it allows
    /// nothing. It lets safe code move a real permission out of a `&mut`
    /// place by putting this one in its stead.
    pub fn empty() -> Self {
        Perm {
            _owns: PhantomData,
            #[cfg(checked)]
            shadow: shadow::Shadow::empty(),
        }
    }
}

fn layout<T>(cap: usize) -> Layout {
    Layout::array::<T>(cap).expect("capacity overflow")
}

/// Allocates a block of `cap` slots.
///
/// ensures: the permission is for the returned address, has `cap` slots,
/// and every slot is uninitialized.
pub fn alloc<T>(cap: usize) -> (Ptr<T>, Perm<T>) {
    let layout = layout::<T>(cap);
    let raw = if layout.size() == 0 {
        NonNull::dangling()
    } else {
        let p = unsafe { alloc::alloc(layout) }.cast::<T>();
        NonNull::new(p).unwrap_or_else(|| alloc::handle_alloc_error(layout))
    };
    let perm = Perm {
        _owns: PhantomData,
        #[cfg(checked)]
        shadow: shadow::Shadow::block(raw.as_ptr() as usize, cap, layout.size() != 0),
    };
    (Ptr { raw }, perm)
}

/// Frees a block, consuming its permission, so rustc rejects any later use.
///
/// requires: `perm` is for `p`, has `cap` slots, and every slot is
/// uninitialized.
pub fn free<T>(p: Ptr<T>, mut perm: Perm<T>, cap: usize) {
    free_in_place(p, &mut perm, cap);
}

/// Frees a block through `&mut`, leaving a permission for nothing behind.
/// `Drop::drop` has only `&mut self`, so it cannot give a permission away.
///
/// requires: as for [`free`]. ensures: `perm` is a permission for nothing.
pub fn free_in_place<T>(p: Ptr<T>, perm: &mut Perm<T>, cap: usize) {
    #[cfg(checked)]
    perm.shadow.free(p.raw.as_ptr() as usize, cap);
    let layout = layout::<T>(cap);
    if layout.size() != 0 {
        unsafe { alloc::dealloc(p.raw.as_ptr().cast(), layout) }
    }
    #[cfg(not(checked))]
    let _ = perm;
}

/// Writes `v` into slot `i`.
///
/// requires: `perm` is for `p`, `i` is one of its slots, and the slot is
/// uninitialized. ensures: the slot holds `v`.
pub fn write<T>(p: Ptr<T>, perm: &mut Perm<T>, i: usize, v: T) {
    #[cfg(checked)]
    perm.shadow.set(p.raw.as_ptr() as usize, i, false, true);
    #[cfg(not(checked))]
    let _ = perm;
    unsafe { p.raw.as_ptr().add(i).write(v) }
}

/// Moves the value out of slot `i`.
///
/// requires: `perm` is for `p`, and slot `i` is initialized.
/// ensures: the slot is uninitialized, and the result is what it held.
pub fn take<T>(p: Ptr<T>, perm: &mut Perm<T>, i: usize) -> T {
    #[cfg(checked)]
    perm.shadow.set(p.raw.as_ptr() as usize, i, true, false);
    #[cfg(not(checked))]
    let _ = perm;
    unsafe { p.raw.as_ptr().add(i).read() }
}

/// Borrows slot `i` for as long as the permission is borrowed.
///
/// requires: `perm` is for `p`, and slot `i` is initialized.
pub fn get<T>(p: Ptr<T>, perm: &Perm<T>, i: usize) -> &T {
    #[cfg(checked)]
    perm.shadow.check_init(p.raw.as_ptr() as usize, i, i + 1);
    #[cfg(not(checked))]
    let _ = perm;
    unsafe { &*p.raw.as_ptr().add(i) }
}

/// Borrows slot `i` mutably for as long as the permission is.
///
/// requires: `perm` is for `p`, and slot `i` is initialized.
/// ensures: the slot stays initialized, holding what the borrow ends with.
pub fn get_mut<T>(p: Ptr<T>, perm: &mut Perm<T>, i: usize) -> &mut T {
    #[cfg(checked)]
    perm.shadow.check_init(p.raw.as_ptr() as usize, i, i + 1);
    #[cfg(not(checked))]
    let _ = perm;
    unsafe { &mut *p.raw.as_ptr().add(i) }
}

/// Borrows slots `0..len` as a Rust slice.
///
/// requires: `perm` is for `p`, and slots `0..len` are initialized.
pub fn slice<T>(p: Ptr<T>, perm: &Perm<T>, len: usize) -> &[T] {
    #[cfg(checked)]
    perm.shadow.check_init(p.raw.as_ptr() as usize, 0, len);
    #[cfg(not(checked))]
    let _ = perm;
    unsafe { std::slice::from_raw_parts(p.raw.as_ptr(), len) }
}

/// Borrows slots `0..len` as a mutable Rust slice.
///
/// requires: `perm` is for `p`, and slots `0..len` are initialized.
pub fn slice_mut<T>(p: Ptr<T>, perm: &mut Perm<T>, len: usize) -> &mut [T] {
    #[cfg(checked)]
    perm.shadow.check_init(p.raw.as_ptr() as usize, 0, len);
    #[cfg(not(checked))]
    let _ = perm;
    unsafe { std::slice::from_raw_parts_mut(p.raw.as_ptr(), len) }
}

/// Moves slots `0..n` of one block into another.
///
/// requires: each permission is for its address; slots `0..n` of `from` are
/// initialized and slots `0..n` of `to` are not.
/// ensures: the two ranges have exchanged those states.
pub fn move_slots<T>(
    from: Ptr<T>,
    from_perm: &mut Perm<T>,
    to: Ptr<T>,
    to_perm: &mut Perm<T>,
    n: usize,
) {
    #[cfg(checked)]
    for i in 0..n {
        from_perm
            .shadow
            .set(from.raw.as_ptr() as usize, i, true, false);
        to_perm.shadow.set(to.raw.as_ptr() as usize, i, false, true);
    }
    #[cfg(not(checked))]
    let _ = (from_perm, to_perm);
    unsafe { std::ptr::copy_nonoverlapping(from.raw.as_ptr(), to.raw.as_ptr(), n) }
}

/// What the checked build counted: blocks allocated, blocks freed, and
/// permissions dropped while they still held a block.
#[cfg(checked)]
pub fn counts() -> (usize, usize, usize) {
    shadow::counts()
}

#[cfg(checked)]
mod shadow {
    //! What a proof would know about a permission, kept at run time.
    use std::sync::atomic::{AtomicUsize, Ordering};

    static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
    static FREED: AtomicUsize = AtomicUsize::new(0);
    static LEAKED: AtomicUsize = AtomicUsize::new(0);

    pub fn counts() -> (usize, usize, usize) {
        (
            ALLOCATED.load(Ordering::SeqCst),
            FREED.load(Ordering::SeqCst),
            LEAKED.load(Ordering::SeqCst),
        )
    }

    pub struct Shadow {
        /// `None` once the block is freed, and for a permission for nothing.
        block: Option<Block>,
    }

    struct Block {
        addr: usize,
        init: Vec<bool>,
        /// False for a block of size zero, which the allocator never saw.
        real: bool,
    }

    impl Shadow {
        pub fn empty() -> Self {
            Shadow { block: None }
        }

        pub fn block(addr: usize, cap: usize, real: bool) -> Self {
            if real {
                ALLOCATED.fetch_add(1, Ordering::SeqCst);
            }
            Shadow {
                block: Some(Block {
                    addr,
                    init: vec![false; cap],
                    real,
                }),
            }
        }

        fn of(&self, addr: usize) -> &Block {
            let block = self.block.as_ref().expect("the permission holds no block");
            assert_eq!(block.addr, addr, "the permission is for another address");
            block
        }

        pub fn check_init(&self, addr: usize, from: usize, to: usize) {
            let block = self.of(addr);
            assert!(to <= block.init.len(), "slot out of the block");
            assert!(
                block.init[from..to].iter().all(|i| *i),
                "slot is not initialized"
            );
        }

        pub fn set(&mut self, addr: usize, i: usize, was: bool, now: bool) {
            self.of(addr);
            let block = self.block.as_mut().unwrap();
            assert!(i < block.init.len(), "slot out of the block");
            assert_eq!(block.init[i], was, "slot is in the wrong state");
            block.init[i] = now;
        }

        pub fn free(&mut self, addr: usize, cap: usize) {
            let block = self.of(addr);
            assert_eq!(block.init.len(), cap, "wrong capacity");
            assert!(
                block.init.iter().all(|i| !*i),
                "freeing a block that still holds values"
            );
            if block.real {
                FREED.fetch_add(1, Ordering::SeqCst);
            }
            self.block = None;
        }
    }

    impl Drop for Shadow {
        fn drop(&mut self) {
            if self.block.as_ref().is_some_and(|b| b.real) {
                LEAKED.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
}
