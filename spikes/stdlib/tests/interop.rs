use locus_stdlib_spike::{
    btree::PBTreeMap,
    checked::{CheckedMap, CheckedVec, inside},
    hash::PHashMap,
    logic::{HashEq, LawfulEq, Observe, TotalOrd},
    vector::PVec,
    wrappers::{WBTreeMap, WHashMap, WVec},
};
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn checked_vector_calls_and_native_sort_handoff() {
    let mut p = PVec::new();
    let mut w = WVec::new();
    for x in [9_i32, 3, 3, 1] {
        let _e = p.push_checked(x);
        let _e = w.push_checked(x);
    }
    assert_eq!(*p.get_checked(1, inside(1, p.len()).unwrap()).0, 3);
    assert_eq!(*w.get_checked(1, inside(1, w.len()).unwrap()).0, 3);
    let mut native = p.into_std();
    native.sort();
    native.dedup();
    w.with_native(|v| {
        v.sort();
        v.dedup();
    });
    assert_eq!(native, w.as_slice());
    assert_eq!(native, vec![1, 3, 9]);
    assert_eq!(w.pop_checked().0, Some(9));
    let restored = PVec::from_std(w.into_std());
    assert_eq!(restored.as_slice(), &[1, 3]);
}
#[test]
fn checked_maps_share_the_same_client() {
    fn client<M: CheckedMap<u32, u32>>(mut map: M) {
        assert_eq!(map.insert_checked(4, 11).0, None);
        assert_eq!(map.insert_checked(4, 12).0, Some(11));
        assert_eq!(map.get_checked(&4).0, Some(&12));
        assert_eq!(map.remove_checked(&4).0, Some(12));
        assert_eq!(map.get_checked(&4).0, None);
    }
    client(PBTreeMap::new());
    client(PHashMap::new());
    client(WBTreeMap::new());
    client(WHashMap::new());
}
#[derive(Debug, Clone)]
struct Key {
    raw: u32,
    label: &'static str,
}
impl Observe for Key {
    type Model = u32;
    fn observe(&self) -> u32 {
        self.raw % 100
    }
}
impl LawfulEq for Key {
    fn eq_checked(&self, o: &Self) -> bool {
        self.observe() == o.observe()
    }
}
impl TotalOrd for Key {
    fn cmp_checked(&self, o: &Self) -> Ordering {
        self.observe().cmp(&o.observe())
    }
}
impl HashEq for Key {
    fn hash_checked(&self) -> u64 {
        self.observe() as u64
    }
}
impl PartialEq for Key {
    fn eq(&self, o: &Self) -> bool {
        self.eq_checked(o)
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Key {
    fn cmp(&self, o: &Self) -> Ordering {
        self.cmp_checked(o)
    }
}
impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.observe().hash(h)
    }
}
#[test]
fn canonical_key_equality_and_representative_retention() {
    let first = Key {
        raw: 107,
        label: "original",
    };
    let later = Key {
        raw: 7,
        label: "replacement",
    };
    let mut t = PBTreeMap::new();
    let mut h = PHashMap::new();
    let mut wt = WBTreeMap::new();
    let mut wh = WHashMap::new();
    t.insert_checked(first.clone(), 1);
    h.insert_checked(first.clone(), 1);
    wt.insert_checked(first.clone(), 1);
    wh.insert_checked(first.clone(), 1);
    assert_eq!(t.insert_checked(later.clone(), 2).0, Some(1));
    assert_eq!(h.insert_checked(later.clone(), 2).0, Some(1));
    assert_eq!(wt.insert_checked(later.clone(), 2).0, Some(1));
    assert_eq!(wh.insert_checked(later.clone(), 2).0, Some(1));
    t.visit(|k, v| {
        assert_eq!((k.raw, k.label, *v), (107, "original", 2));
    });
    h.visit(|k, v| {
        assert_eq!((k.raw, k.label, *v), (107, "original", 2));
    });
    let native = wt.into_std();
    assert_eq!(native.get_key_value(&later).unwrap().0.raw, 107);
    assert_eq!(
        wh.into_std().get_key_value(&later).unwrap().0.label,
        "original"
    );
}
#[test]
fn btree_to_std_to_wrapped_to_native_hash() {
    let mut source = PBTreeMap::new();
    for k in 0..40 {
        source.insert(k, k * k);
    }
    let native: BTreeMap<_, _> = source.into_entries().into_std().into_iter().collect();
    let mut wrapped = WBTreeMap::from_std(native);
    wrapped.with_native(|m| {
        m.retain(|k, _| k % 2 == 0);
    });
    let mut hash = PHashMap::new();
    for (k, v) in wrapped.into_std() {
        hash.insert(k, v);
    }
    for k in 0..40 {
        assert_eq!(
            hash.get(&k).copied(),
            if k % 2 == 0 { Some(k * k) } else { None }
        );
    }
}
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct BadOrd(u32);
impl Observe for BadOrd {
    type Model = u32;
    fn observe(&self) -> u32 {
        self.0 % 100
    }
}
impl LawfulEq for BadOrd {
    fn eq_checked(&self, o: &Self) -> bool {
        self.observe() == o.observe()
    }
}
impl TotalOrd for BadOrd {
    fn cmp_checked(&self, o: &Self) -> Ordering {
        self.observe().cmp(&o.observe())
    }
}
#[test]
fn a_law_trait_does_not_automatically_bridge_std_ord() {
    let mut native = PBTreeMap::new();
    assert_eq!(native.insert(BadOrd(107), 1), None);
    assert_eq!(native.insert(BadOrd(7), 2), Some(1));
    let mut wrapper = WBTreeMap::new();
    wrapper.insert(BadOrd(107), 1);
    wrapper.insert(BadOrd(7), 2);
    assert_eq!(wrapper.len(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| wrapper.validate())).is_err());
}
struct BadHash(u32);
impl Observe for BadHash {
    type Model = u32;
    fn observe(&self) -> u32 {
        self.0 % 100
    }
}
impl LawfulEq for BadHash {
    fn eq_checked(&self, o: &Self) -> bool {
        self.observe() == o.observe()
    }
}
impl HashEq for BadHash {
    fn hash_checked(&self) -> u64 {
        self.0 as u64
    }
}
#[test]
fn equal_keys_with_different_hashes_break_the_map_model() {
    let mut map = PHashMap::new();
    map.insert(BadHash(107), 1);
    map.insert(BadHash(7), 2);
    assert_eq!(map.len(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| map.observe())).is_err());
    // An actual Locus HashEq implementation must fail its hash-code proof.
}
#[test]
fn forgotten_native_drain_is_reobserved_not_assumed_preserving() {
    let mut w = WVec::from_std(vec![1_u32, 2, 3, 4]);
    w.with_native(|raw| std::mem::forget(raw.drain(1..3)));
    // Rust documents that a forgotten Drain may lose additional elements.
    w.validate();
    assert_eq!(w.as_slice(), &[1]);
}
#[test]
fn stale_bound_receipt_is_rejected() {
    let mut p = PVec::new();
    p.push(1_u32);
    p.push(2);
    let bound = inside(1, p.len()).unwrap();
    p.pop();
    assert!(catch_unwind(AssertUnwindSafe(|| p.get_checked(1, bound))).is_err());
}
