//! Executable Rust observations, not a Locus verifier or proof implementation.
#![allow(dead_code, dropping_references, forgetting_references)]
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
// Unchanged permission kernel vendored from the previous stdlib spike.
// These compatibility types are runtime audit data, never Locus logic/proofs.
mod logic {
    pub trait Observe {
        type Model;
        fn observe(&self) -> Self::Model;
    }
    pub struct Seq<T>(pub Vec<T>);
}
#[path = "permission_kernel.rs"]
mod permission;

struct NativeVec<T> {
    storage: permission::Region<T>,
    len: usize,
}
impl<T> NativeVec<T> {
    fn two(a: T, b: T) -> Self {
        let mut storage = permission::Region::allocate(3);
        let p = storage.pointer();
        storage.write(p, 0, a);
        storage.write(p, 1, b);
        Self { storage, len: 2 }
    }
    fn valid(&self) -> bool {
        self.len <= self.storage.capacity()
            && (0..self.storage.capacity()).all(|i| self.storage.initialized(i) == (i < self.len))
    }
    fn get_mut(&mut self, index: usize) -> &mut T {
        assert!(index < self.len); // Runtime surrogate for the caller's proof.
        let p = self.storage.pointer();
        let r = &mut self.storage;
        let boundary = AbortOnUnwind::new();
        let result = r.borrow_mut(p, index);
        boundary.finish();
        result
    }
}
impl<T> Drop for NativeVec<T> {
    fn drop(&mut self) {
        eprintln!("NATIVE_OWNER_DROP_VALID={}", self.valid());
    }
}

struct AbortOnUnwind(bool);
impl AbortOnUnwind {
    fn new() -> Self {
        Self(true)
    }
    fn finish(mut self) {
        self.0 = false;
    }
}
impl Drop for AbortOnUnwind {
    fn drop(&mut self) {
        if self.0 && std::thread::panicking() {
            eprintln!("ABORT_ACQUISITION");
            std::process::abort();
        }
    }
}

struct Region {
    slots: Vec<Option<i32>>,
}
impl Region {
    fn len(&self) -> usize {
        self.slots.len()
    }
    fn slot_mut(&mut self, i: usize) -> &mut i32 {
        self.slots[i].as_mut().expect("initialized slot")
    }
    // A handwritten method shim: arguments are evaluated before entry, while
    // method autoref retains Rust's two-phase-borrow behavior. This demonstrates
    // a possible boundary; it is not a generic source-to-source lowering.
    fn guarded_slot_mut(&mut self, i: usize) -> &mut i32 {
        let boundary = AbortOnUnwind::new();
        let x = self.slot_mut(i);
        boundary.finish();
        x
    }
}

struct Owner {
    storage: Region,
    len: usize,
}
impl Owner {
    fn new() -> Self {
        Self {
            storage: Region {
                slots: vec![Some(10), Some(20), None],
            },
            len: 2,
        }
    }
    // Runtime audit of the proposed proof-field invariant, not erased evidence.
    fn valid(&self) -> bool {
        self.len <= self.storage.slots.len()
            && self
                .storage
                .slots
                .iter()
                .enumerate()
                .all(|(i, x)| x.is_some() == (i < self.len))
    }
    fn get_mut(&mut self, i: usize) -> &mut i32 {
        self.storage.guarded_slot_mut(i)
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        eprintln!("OWNER_DROP_VALID={}", self.valid());
    }
}

fn protected<F: FnOnce() -> R, R>(f: F) -> R {
    let boundary = AbortOnUnwind::new();
    let result = f();
    boundary.finish();
    result
}

struct TraceDrop;
impl Drop for TraceDrop {
    fn drop(&mut self) {
        eprintln!("CALLEE_LOCAL_DROP");
    }
}
struct PanicDrop;
impl Drop for PanicDrop {
    fn drop(&mut self) {
        panic!("result destructor");
    }
}

// Intentionally bad: restoring in Drop is not enough, because safe Rust permits
// forgetting the guard. No unsafe code or invalid Rust value is involved.
struct RepairGuard<'a> {
    place: &'a mut Option<i32>,
    saved: Option<i32>,
}
impl<'a> RepairGuard<'a> {
    fn take(place: &'a mut Option<i32>) -> Self {
        let saved = place.take();
        Self { place, saved }
    }
}
impl Drop for RepairGuard<'_> {
    fn drop(&mut self) {
        *self.place = self.saved.take();
    }
}

// Intentionally bad deferred acquisition: the constructor returns synchronously,
// but polling suspends with an invalid owner. Cancellation never restores it.
struct Deferred<'a> {
    place: &'a mut Option<i32>,
}
impl Future for Deferred<'_> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        *self.place = None;
        Poll::Pending
    }
}

fn scenario(name: &str) {
    match name {
        "success" => {
            let mut owner = Owner::new();
            *owner.get_mut(0) = 99;
            assert!(owner.valid());
            assert_eq!(owner.storage.slots[0], Some(99));
            println!("SUCCESS");
        }
        "forget_reference" => {
            let mut owner = Owner::new();
            let x = owner.get_mut(0);
            *x = 99;
            std::mem::forget(x);
            assert!(owner.valid());
            println!("NO_REPAIR_DROP_NEEDED");
        }
        "forget_repair_guard" => {
            let mut owner = Owner::new();
            std::mem::forget(RepairGuard::take(&mut owner.storage.slots[0]));
            assert!(!owner.valid());
            println!("COUNTEREXAMPLE_FORGET");
        }
        "panic_acquire_changed" | "panic_acquire_unchanged" | "callee_cleanup" => {
            let mut owner = Owner::new();
            let _ = catch_unwind(AssertUnwindSafe(|| {
                protected(|| {
                    let _cleanup = TraceDrop;
                    if name != "panic_acquire_unchanged" {
                        owner.storage.slots[0] = None;
                    }
                    panic!("acquisition failure");
                })
            }));
            panic!("must abort before catch or owner drop");
        }
        "panic_after_return" => {
            let mut owner = Owner::new();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    *owner.get_mut(0) = 31;
                    panic!("client panics after a successful acquisition");
                }))
                .is_err()
            );
            assert!(owner.valid());
            println!("CAUGHT_AFTER_RETURN");
        }
        "panic_argument_outside" => {
            let mut owner = Owner::new();
            let argument = || -> usize {
                panic!("argument evaluation");
            };
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    owner.storage.guarded_slot_mut(argument());
                }))
                .is_err()
            );
            assert!(owner.valid());
            println!("CAUGHT_ARGUMENT");
        }
        "panic_argument_broad_guard" => {
            let mut owner = Owner::new();
            let argument = || -> usize {
                panic!("argument evaluation");
            };
            let _ = catch_unwind(AssertUnwindSafe(|| {
                let _boundary = AbortOnUnwind::new();
                owner.storage.slot_mut(argument());
            }));
            panic!("broad expression guard changed catch into abort");
        }
        "result_drop" => {
            let owner = Owner::new();
            assert!(
                catch_unwind(|| {
                    let x = protected(|| PanicDrop);
                    drop(x);
                })
                .is_err()
            );
            assert!(owner.valid());
            println!("CAUGHT_RESULT_DROP");
        }
        "deferred_closure" => {
            let mut owner = Owner::new();
            let late = protected(|| {
                let place = &mut owner.storage.slots[0];
                move || {
                    *place = None;
                    panic!("deferred mutation");
                }
            });
            assert!(catch_unwind(AssertUnwindSafe(late)).is_err());
            assert!(!owner.valid());
            println!("COUNTEREXAMPLE_DEFERRED_CLOSURE");
        }
        "future_cancellation" => {
            let mut owner = Owner::new();
            let future = protected(|| Deferred {
                place: &mut owner.storage.slots[0],
            });
            let mut future = Box::pin(future);
            assert!(
                future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            drop(future);
            assert!(!owner.valid());
            println!("COUNTEREXAMPLE_CANCELLATION");
        }
        "two_phase" => {
            let mut region = Region {
                slots: vec![Some(1), Some(2)],
            };
            *region.guarded_slot_mut(region.len() - 1) = 8;
            assert_eq!(region.slots, vec![Some(1), Some(8)]);
            println!("TWO_PHASE_METHOD_SHIM");
        }
        "named_acquisition" => {
            let mut owner = Owner::new();
            let index = owner.len - 1;
            let r = &mut owner.storage;
            let boundary = AbortOnUnwind::new();
            let x = r.slot_mut(index); // r's single runtime use
            boundary.finish();
            *x = 77;
            assert!(owner.valid());
            println!("NAMED_ACQUISITION");
        }
        "named_argument_panic" => {
            let mut owner = Owner::new();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let r = &mut owner.storage;
                    // Active parent borrow, but no mutation occurred through it.
                    // Method shim begins the abort boundary after argument evaluation.
                    let argument = || -> usize {
                        panic!("named argument evaluation");
                    };
                    r.guarded_slot_mut(argument());
                }))
                .is_err()
            );
            assert!(owner.valid());
            println!("CAUGHT_NAMED_ARGUMENT");
        }
        "native_panic_acquire" => {
            let mut owner = NativeVec::two(1, 2);
            let p = owner.storage.pointer();
            let _ = catch_unwind(AssertUnwindSafe(|| {
                protected(|| {
                    // Deliberately invalid acquisition: its normal-return signature
                    // alone would not protect this unwind. No invalid Rust value.
                    let _ = owner.storage.take(p, 0);
                    owner.storage.borrow_mut(p, 0);
                })
            }));
            panic!("must abort before native owner destruction");
        }
        "native_panic_after_return" => {
            let mut owner = NativeVec::two(String::from("a"), String::from("b"));
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    owner.get_mut(0).push_str(" changed");
                    panic!("client panic with native element loan");
                }))
                .is_err()
            );
            assert!(owner.valid());
            assert_eq!(owner.storage.read(owner.storage.pointer(), 0), "a changed");
            println!("CAUGHT_NATIVE_AFTER_RETURN");
        }
        _ => panic!("unknown scenario: {name}"),
    }
}

fn main() {
    scenario(&std::env::args().nth(1).expect("scenario"));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inner_and_outer_end_values_differ() {
        let mut value = 0;
        let outer = &mut value;
        let inner = &mut *outer;
        *inner = 1;
        let inner_end = *inner;
        *outer = 2;
        let outer_end = *outer;
        assert_eq!((inner_end, outer_end, value), (1, 2, 2));
    }
    #[test]
    fn reference_retargeting_does_not_retarget_original_loan() {
        let (mut original, mut other) = (0, 7);
        let mut r = &mut original;
        *r = 1;
        r = &mut other;
        *r = 9;
        assert_eq!((original, other), (1, 9));
    }
    #[test]
    fn reference_slot_is_different_from_initial_referent() {
        fn replace<'a>(r: &mut &'a mut i32, next: &'a mut i32) {
            *r = next;
        }
        let (mut a, mut b) = (1, 2);
        let mut r = &mut a;
        replace(&mut r, &mut b);
        *r = 3;
        assert_eq!((a, b), (1, 3));
    }
    #[test]
    fn reborrow_returned_from_call_ends_before_parent() {
        fn lend(x: &mut i32) -> &mut i32 {
            x
        }
        let mut value = 0;
        let r = &mut value;
        let child = lend(&mut *r);
        *child = 4;
        let call_loan_end = *child;
        *r = 5;
        assert_eq!((call_loan_end, value), (4, 5));
    }
    #[test]
    fn rust_allows_sibling_proof_read_so_locus_must_check_it() {
        struct FakeProofOwner {
            data: i32,
            proof_marker: bool,
        }
        let mut owner = FakeProofOwner {
            data: 1,
            proof_marker: true,
        };
        let r = &mut owner.data;
        let stale_evidence = owner.proof_marker;
        *r = -1;
        assert!(stale_evidence && owner.data < 0);
    }
    #[test]
    fn two_disjoint_loans_can_break_one_joint_invariant() {
        let mut pair = (0, 0);
        let a = &mut pair.0;
        let b = &mut pair.1;
        *a = 5;
        *b = 6;
        assert!(pair.0 + pair.1 > 10);
    }
    #[test]
    fn initialized_prefix_preserved_for_every_tested_element_write() {
        for index in 0..2 {
            for value in [i32::MIN, -1, 0, 1, i32::MAX] {
                let mut owner = Owner::new();
                *owner.get_mut(index) = value;
                assert!(owner.valid());
            }
        }
    }
    #[test]
    fn same_element_write_breaks_sorted_invariant() {
        let mut owner = Owner::new();
        *owner.get_mut(0) = 99;
        assert!(owner.valid());
        assert!(owner.storage.slots[0] > owner.storage.slots[1]);
    }
    #[test]
    fn captured_index_denotes_original_place() {
        let mut values = [10, 20];
        let mut index = 0;
        let r = &mut values[index];
        index = 1;
        *r = 99;
        assert_eq!((values, index), ([99, 20], 1));
    }
    #[test]
    fn swap_of_returned_reference_does_not_swap_loan_identity() {
        let mut owner = Owner::new();
        let mut other = 7;
        let mut x = owner.get_mut(0);
        let mut y = &mut other;
        std::mem::swap(&mut x, &mut y);
        *x = 8;
        *y = 9;
        assert_eq!(other, 8);
        assert_eq!(owner.storage.slots[0], Some(9));
        assert!(owner.valid());
    }
    #[test]
    fn guarded_returned_loan_may_escape_through_another_parameter() {
        fn lend<'a>(r: &'a mut Region, out: &mut Option<&'a mut i32>) {
            *out = Some(r.slot_mut(0));
        }
        let mut owner = Owner::new();
        let mut out = None;
        let r = &mut owner.storage;
        let boundary = AbortOnUnwind::new();
        lend(r, &mut out);
        boundary.finish();
        *out.unwrap() = 88;
        assert!(owner.valid());
    }
    #[test]
    fn native_reference_forget_preserves_initialization() {
        let mut owner = NativeVec::two(String::from("a"), String::from("b"));
        let r = owner.get_mut(1);
        r.push('!');
        std::mem::forget(r);
        assert!(owner.valid());
        assert_eq!(owner.storage.read(owner.storage.pointer(), 1), "b!");
    }
    #[test]
    fn native_caller_unwind_drops_each_value_once() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Count(Arc<AtomicUsize>, bool);
        impl Drop for Count {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let count = Arc::new(AtomicUsize::new(0));
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut owner =
                    NativeVec::two(Count(count.clone(), false), Count(count.clone(), false));
                owner.get_mut(0).1 = true;
                panic!("unwind destroys native vector after loan");
            }))
            .is_err()
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn native_element_drop_panic_still_cleans_remaining_values() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct Count(Arc<AtomicUsize>, bool);
        impl Drop for Count {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
                if self.1 {
                    panic!("first element destructor");
                }
            }
        }
        let count = Arc::new(AtomicUsize::new(0));
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut owner =
                    NativeVec::two(Count(count.clone(), false), Count(count.clone(), false));
                owner.get_mut(0).1 = true;
                drop(owner);
            }))
            .is_err()
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn native_zero_sized_element_loan() {
        let mut owner = NativeVec::two((), ());
        let r = owner.get_mut(1);
        *r = ();
        assert!(owner.valid());
    }
}
