#[cfg(feature = "audit")]
use locus_stdlib_spike::logic::Seq;
use locus_stdlib_spike::{
    interior::{PCell, PMutex, PRefCell, WCell, WMutex, WRefCell},
    logic::Observe,
    sharing::{PArc, PRc, WArc, WRc},
    vector::PVec,
    wrappers::WVec,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[test]
fn cells_change_payload_without_changing_root_observation() {
    let a = PCell::new(4_u32);
    let b = WCell::new(4_u32);
    let old_a = a.observe();
    let old_b = b.observe();
    assert_eq!(a.replace(9), 4);
    assert_eq!(b.replace(9), 4);
    assert_eq!((a.get(), b.get()), (9, 9));
    assert_eq!(old_a, a.observe());
    assert_eq!(old_b, b.observe());
}
#[test]
fn refcell_shared_exclusive_and_forgotten_guards() {
    let p = PRefCell::new(5_u32);
    let w = WRefCell::new(5_u32);
    let a = p.try_borrow().unwrap();
    let b = p.try_borrow().unwrap();
    assert!(p.try_borrow_mut().is_none());
    assert_eq!((*a, *b), (5, 5));
    drop((a, b));
    {
        let mut g = p.try_borrow_mut().unwrap();
        *g = 8;
        assert!(p.try_borrow().is_none());
    }
    {
        let mut g = w.try_borrow_mut().unwrap();
        *g = 8;
        assert!(w.try_borrow().is_none());
    }
    assert_eq!(*p.try_borrow().unwrap(), 8);
    let g = w.try_borrow().unwrap();
    assert_eq!(*g, 8);
    #[cfg(feature = "audit")]
    assert_eq!(g.model(), Some(&8));
    drop(g);
    std::mem::forget(p.try_borrow_mut().unwrap());
    std::mem::forget(w.try_borrow_mut().unwrap());
    assert!(p.try_borrow().is_none());
    assert!(w.try_borrow().is_none());
}
struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn rc_count_unique_access_and_last_drop() {
    let count = Arc::new(AtomicUsize::new(0));
    let mut a = PRc::new(Dropped(count.clone()));
    let b = a.clone();
    assert_eq!(a.strong_count(), 2);
    assert!(a.get_mut().is_none());
    drop(b);
    assert!(a.get_mut().is_some());
    drop(a);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let a = WRc::new(11_u32);
    let b = a.clone();
    let native = b.to_std();
    assert_eq!(*native, 11);
    assert_eq!(a.strong_count(), 3);
    #[cfg(feature = "audit")]
    assert_eq!(a.snapshot(), Some(&11));
}
#[test]
fn arc_concurrent_clones_release_exactly_once() {
    let count = Arc::new(AtomicUsize::new(0));
    let a = PArc::new(Dropped(count.clone()));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let a = a.clone();
        threads.push(std::thread::spawn(move || {
            for _ in 0..2000 {
                let b = a.clone();
                let c = b.clone();
                drop(c);
                drop(b);
            }
        }));
    }
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(a.strong_count(), 1);
    drop(a);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
#[test]
fn both_mutexes_synchronize_shared_updates() {
    let p = PArc::new(PMutex::new(0_u64));
    let w = WArc::new(WMutex::new(0_u64));
    let mut threads = Vec::new();
    for _ in 0..6 {
        let p = p.clone();
        let w = w.clone();
        threads.push(std::thread::spawn(move || {
            for _ in 0..1500 {
                *p.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                *w.get().lock().unwrap_or_else(|e| e.into_inner()) += 1;
            }
        }));
    }
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(*p.lock().unwrap_or_else(|e| e.into_inner()), 9000);
    let native = w.to_std();
    assert_eq!(*native.lock().unwrap_or_else(|e| e.into_inner()), 9000);
}
#[test]
fn mutex_poison_preserves_valid_data_and_forget_keeps_lock() {
    let p = PMutex::new(3_u32);
    let w = WMutex::new(3_u32);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut g = p.lock().unwrap_or_else(|e| e.into_inner());
            *g = 8;
            panic!("inside lock")
        }))
        .is_err()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut g = w.lock().unwrap_or_else(|e| e.into_inner());
            *g = 8;
            panic!("inside lock")
        }))
        .is_err()
    );
    assert!(p.is_poisoned() && w.is_poisoned());
    assert_eq!(*p.lock().unwrap_or_else(|e| e.into_inner()), 8);
    assert_eq!(*w.lock().unwrap_or_else(|e| e.into_inner()), 8);
    std::mem::forget(p.lock().unwrap_or_else(|e| e.into_inner()));
    std::mem::forget(w.lock().unwrap_or_else(|e| e.into_inner()));
    assert!(p.try_lock().is_none());
    assert!(!w.try_lock());
}
#[test]
fn wrapped_vector_forgotten_guard_is_fail_closed() {
    let mut w = WVec::from_std(vec![1_u32]);
    let mut guard = w.loan();
    guard[0] = 9;
    std::mem::forget(guard);
    assert_eq!(w.as_slice(), &[9]);
    assert!(w.snapshot().is_none());
    w.refresh();
    #[cfg(feature = "audit")]
    assert_eq!(w.snapshot(), Some(&Seq(vec![9])));
}
#[test]
fn destructor_only_restoration_has_a_counterexample() {
    struct Bad {
        raw: Vec<u32>,
        model: Vec<u32>,
    }
    struct Loan<'a>(&'a mut Bad);
    impl Drop for Loan<'_> {
        fn drop(&mut self) {
            self.0.model = self.0.raw.clone();
        }
    }
    let mut bad = Bad {
        raw: vec![1],
        model: vec![1],
    };
    let loan = Loan(&mut bad);
    loan.0.raw[0] = 9;
    std::mem::forget(loan);
    assert_eq!(bad.raw, vec![9]);
    assert_eq!(bad.model, vec![1]);
    assert_ne!(bad.raw, bad.model);
}
#[test]
fn closed_native_handoff_restores_after_unwind() {
    let mut w = WVec::from_std(vec![3_i32, 1]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| w.with_native(|raw| {
            raw.push(2);
            raw.sort();
            panic!("after native mutation")
        })))
        .is_err()
    );
    assert_eq!(w.as_slice(), &[1, 2, 3]);
    w.validate();
    #[cfg(feature = "audit")]
    assert_eq!(w.snapshot(), Some(&Seq(vec![1, 2, 3])));
}
#[test]
fn observing_failure_cannot_leave_stale_shadow_valid() {
    struct MayFail(u32);
    impl Observe for MayFail {
        type Model = u32;
        fn observe(&self) -> u32 {
            assert!(self.0 != 9, "observation failed");
            self.0
        }
    }
    let mut w = WVec::from_std(vec![MayFail(1)]);
    let result = catch_unwind(AssertUnwindSafe(|| w.with_native(|raw| raw[0].0 = 9)));
    #[cfg(feature = "audit")]
    {
        assert!(result.is_err());
        assert!(w.snapshot().is_none());
    }
    #[cfg(not(feature = "audit"))]
    assert!(result.is_ok());
}
#[test]
fn permission_cleanup_continues_after_one_destructor_panics() {
    struct Bomb {
        n: usize,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Bomb {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
            if self.n == 1 {
                panic!("one destructor");
            }
        }
    }
    let count = Arc::new(AtomicUsize::new(0));
    let mut p = PVec::new();
    for n in 0..4 {
        p.push(Bomb {
            n,
            drops: count.clone(),
        });
    }
    assert!(catch_unwind(AssertUnwindSafe(|| drop(p))).is_err());
    assert_eq!(count.load(Ordering::SeqCst), 4);
}
