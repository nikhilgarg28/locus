use locus_stdlib_spike::{
    btree::PBTreeMap,
    hash::PHashMap,
    logic::{HashEq, LawfulEq, Map, Observe, Seq, TotalOrd},
    permission::{PBox, Region},
    vector::PVec,
    wrappers::{WBTreeMap, WBox, WHashMap, WVec},
};
use std::{
    collections::{BTreeMap, HashMap},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn random(s: &mut u64) -> u64 {
    *s = s
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *s
}

#[test]
fn permission_rejects_wrong_region_uninitialized_and_overwrite() {
    let mut a = Region::<u32>::allocate(2);
    let b = Region::<u32>::allocate(2);
    assert!(catch_unwind(AssertUnwindSafe(|| a.write(b.pointer(), 0, 1))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| a.take(a.pointer(), 0))).is_err());
    a.write(a.pointer(), 0, 7);
    assert!(catch_unwind(AssertUnwindSafe(|| a.write(a.pointer(), 0, 8))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| a.write(a.pointer(), 2, 8))).is_err());
    assert_eq!(a.take(a.pointer(), 0), 7);
    assert!(catch_unwind(AssertUnwindSafe(|| a.take(a.pointer(), 0))).is_err());
}
#[test]
fn zero_sized_allocations_have_distinct_authority() {
    let mut a = Region::<()>::allocate(1);
    let b = Region::<()>::allocate(1);
    assert!(catch_unwind(AssertUnwindSafe(|| a.write(b.pointer(), 0, ()))).is_err());
    a.write(a.pointer(), 0, ());
    assert_eq!(a.slice(1), &[()]);
}
#[test]
fn vector_differential_traces() {
    for seed in 1..=24 {
        let mut rng = seed;
        let mut p = PVec::new();
        let mut w = WVec::new();
        let mut s = Vec::new();
        for _ in 0..1500 {
            let x = (random(&mut rng) % 1000) as i32;
            let op = random(&mut rng) % 6;
            match op {
                0 => {
                    p.push(x);
                    w.push(x);
                    s.push(x);
                }
                1 => {
                    let expected = s.pop();
                    assert_eq!(p.pop(), expected);
                    assert_eq!(w.pop(), expected);
                }
                2 => {
                    let i = (random(&mut rng) as usize) % (s.len() + 1);
                    p.insert(i, x);
                    w.insert(i, x);
                    s.insert(i, x);
                }
                3 if !s.is_empty() => {
                    let i = (random(&mut rng) as usize) % s.len();
                    let expected = s.remove(i);
                    assert_eq!(p.remove(i), expected);
                    assert_eq!(w.remove(i), expected);
                }
                4 if !s.is_empty() => {
                    let i = (random(&mut rng) as usize) % s.len();
                    let expected = std::mem::replace(&mut s[i], x);
                    assert_eq!(p.set(i, x), expected);
                    assert_eq!(w.set(i, x), expected);
                }
                _ => {}
            }
            assert_eq!(p.as_slice(), s.as_slice());
            assert_eq!(w.as_slice(), s.as_slice());
            assert_eq!(p.observe(), Seq(s.clone()));
            w.validate();
            #[cfg(feature = "audit")]
            assert_eq!(w.snapshot(), Some(&Seq(s.clone())));
        }
    }
}

#[test]
fn maps_differential_traces() {
    for seed in 1..=12 {
        let mut rng = seed;
        let mut tree = PBTreeMap::new();
        let mut hash = PHashMap::new();
        let mut wt = WBTreeMap::new();
        let mut wh = WHashMap::new();
        let mut oracle = BTreeMap::new();
        for _ in 0..1800 {
            let k = (random(&mut rng) % 80) as i32;
            let v = (random(&mut rng) % 10000) as i64;
            match random(&mut rng) % 3 {
                0 => {
                    let expected = oracle.insert(k, v);
                    assert_eq!(tree.insert(k, v), expected);
                    assert_eq!(hash.insert(k, v), expected);
                    assert_eq!(wt.insert(k, v), expected);
                    assert_eq!(wh.insert(k, v), expected);
                }
                1 => {
                    let expected = oracle.remove(&k);
                    assert_eq!(tree.remove(&k), expected);
                    assert_eq!(hash.remove(&k), expected);
                    assert_eq!(wt.remove(&k), expected);
                    assert_eq!(wh.remove(&k), expected);
                }
                _ => {
                    assert_eq!(tree.get(&k), oracle.get(&k));
                    assert_eq!(hash.get(&k), oracle.get(&k));
                    assert_eq!(wt.get(&k), oracle.get(&k));
                    assert_eq!(wh.get(&k), oracle.get(&k));
                }
            }
            tree.validate();
            hash.validate();
            let mut model = Map::empty();
            for (k, v) in &oracle {
                model.insert(*k, *v);
            }
            assert_eq!(tree.observe(), model);
            assert_eq!(hash.observe(), model);
            assert_eq!(tree.len(), oracle.len());
            assert_eq!(hash.len(), oracle.len());
            #[cfg(feature = "audit")]
            {
                assert_eq!(wt.snapshot(), Some(&model));
                assert_eq!(wh.snapshot(), Some(&model));
            }
        }
        wt.validate();
        wh.validate();
    }
}

fn permutations(a: &mut [i32], i: usize, f: &mut impl FnMut(&[i32])) {
    if i == a.len() {
        f(a);
        return;
    }
    for j in i..a.len() {
        a.swap(i, j);
        permutations(a, i + 1, f);
        a.swap(i, j);
    }
}
#[test]
fn btree_every_six_key_removal_order() {
    let mut count = 0;
    permutations(&mut [0, 1, 2, 3, 4, 5], 0, &mut |order| {
        let mut map = PBTreeMap::new();
        for k in 0..6 {
            map.insert(k, k * 10);
        }
        for k in order {
            assert_eq!(map.remove(k), Some(*k * 10));
            map.validate();
        }
        assert!(map.is_empty());
        count += 1;
    });
    assert_eq!(count, 720);
}
#[derive(Debug)]
struct Collision(u32);
impl Observe for Collision {
    type Model = u32;
    fn observe(&self) -> u32 {
        self.0
    }
}
impl LawfulEq for Collision {
    fn eq_checked(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl HashEq for Collision {
    fn hash_checked(&self) -> u64 {
        0
    }
}
#[test]
fn all_hashes_collide_and_tombstones_do_not_hide_keys() {
    let mut map = PHashMap::new();
    for k in 0..128 {
        assert_eq!(map.insert(Collision(k), k), None);
    }
    for k in (0..128).step_by(2) {
        assert_eq!(map.remove(&Collision(k)), Some(k));
    }
    for k in 0..128 {
        assert_eq!(
            map.get(&Collision(k)).copied(),
            if k % 2 == 0 { None } else { Some(k) }
        );
    }
    for k in 0..128 {
        map.insert(Collision(k), k + 1000);
    }
    map.validate();
    for k in 0..128 {
        assert_eq!(map.get(&Collision(k)), Some(&(k + 1000)));
    }
}
#[derive(Debug)]
struct NoStdTraits(u32);
impl Observe for NoStdTraits {
    type Model = u32;
    fn observe(&self) -> u32 {
        self.0
    }
}
impl LawfulEq for NoStdTraits {
    fn eq_checked(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl TotalOrd for NoStdTraits {
    fn cmp_checked(&self, o: &Self) -> std::cmp::Ordering {
        self.0.cmp(&o.0)
    }
}
impl HashEq for NoStdTraits {
    fn hash_checked(&self) -> u64 {
        self.0 as u64
    }
}
#[test]
fn native_maps_need_no_rust_ord_or_hash() {
    let mut t = PBTreeMap::new();
    let mut h = PHashMap::new();
    t.insert(NoStdTraits(7), 8);
    h.insert(NoStdTraits(7), 9);
    assert_eq!(t.get(&NoStdTraits(7)), Some(&8));
    assert_eq!(h.get(&NoStdTraits(7)), Some(&9));
}

struct Counted {
    id: usize,
    drops: Arc<AtomicUsize>,
}
impl Drop for Counted {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl Observe for Counted {
    type Model = usize;
    fn observe(&self) -> usize {
        self.id
    }
}
#[test]
fn nonclone_elements_drop_exactly_once() {
    let drops = Arc::new(AtomicUsize::new(0));
    {
        let mut p = PVec::new();
        let mut w = WVec::new();
        let mut t = PBTreeMap::new();
        let mut h = PHashMap::new();
        for id in 0..100 {
            p.push(Counted {
                id,
                drops: drops.clone(),
            });
            w.push(Counted {
                id,
                drops: drops.clone(),
            });
            t.insert(
                id,
                Counted {
                    id,
                    drops: drops.clone(),
                },
            );
            h.insert(
                id,
                Counted {
                    id,
                    drops: drops.clone(),
                },
            );
        }
        drop(p.remove(31));
        drop(w.pop());
        drop(t.remove(&33));
        drop(h.remove(&48));
    }
    assert_eq!(drops.load(Ordering::SeqCst), 400);
}
#[test]
fn boxes_move_without_clone_and_round_trip() {
    let mut p = PBox::new(String::from("owned"));
    p.get_mut().push('!');
    assert_eq!(p.into_inner(), "owned!");
    let mut w = WBox::new(String::from("wrapped"));
    w.with_mut(|s| s.push('!'));
    #[cfg(feature = "audit")]
    assert_eq!(w.snapshot(), Some(&String::from("wrapped!")));
    assert_eq!(*w.into_std(), "wrapped!");
}
#[test]
fn map_equality_ignores_insertion_order() {
    let mut a = Map::empty();
    let mut b = Map::empty();
    for k in 0..8 {
        a.insert(k, k * k);
    }
    for k in (0..8).rev() {
        b.insert(k, k * k);
    }
    assert_eq!(a, b);
    b.insert(3, 0);
    assert_ne!(a, b);
}
#[test]
fn native_map_consuming_conversion_keeps_every_value() {
    let mut t = PBTreeMap::new();
    let mut h = PHashMap::new();
    for k in 0..70 {
        t.insert(k, k + 1);
        h.insert(k, k + 1);
    }
    let tree: BTreeMap<_, _> = t.into_entries().into_std().into_iter().collect();
    let hash: HashMap<_, _> = h.into_entries().into_std().into_iter().collect();
    for k in 0..70 {
        assert_eq!(tree.get(&k), Some(&(k + 1)));
        assert_eq!(hash.get(&k), Some(&(k + 1)));
    }
}
