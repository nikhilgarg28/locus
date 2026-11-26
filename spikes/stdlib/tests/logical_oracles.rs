//! Finite algebra checks of runtime oracles, not proofs of the Locus definitions.
use locus_stdlib_spike::logic::{Map, Seq};

#[test]
fn sequence_update_algebra_on_small_domains() {
    for len in 0..7 {
        for bits in 0..(1_usize << len) {
            let original = Seq((0..len).map(|i| ((bits >> i) & 1) as u32).collect());
            for i in 0..len + 2 {
                let mut updated = original.clone();
                updated.update(i, 9);
                assert_eq!(updated.len(), original.len());
                for j in 0..len + 2 {
                    if i == j && i < len {
                        assert_eq!(updated.get(j), Some(&9));
                    } else {
                        assert_eq!(updated.get(j), original.get(j));
                    }
                }
            }
        }
    }
}

#[test]
fn finite_map_update_remove_and_extensionality() {
    for bits in 0..16 {
        let mut original = Map::empty();
        for k in 0..4 {
            if bits & (1 << k) != 0 {
                original.insert(k, k + 10);
            }
        }
        for k in 0..4 {
            let mut updated = original.clone();
            let old = updated.insert(k, 99);
            assert_eq!(old.as_ref(), original.get(&k));
            assert_eq!(updated.len(), original.len() + usize::from(old.is_none()));
            for q in 0..4 {
                assert_eq!(
                    updated.get(&q),
                    if q == k { Some(&99) } else { original.get(&q) }
                );
            }
            assert_eq!(updated.remove(&k), Some(99));
            let mut removed = original.clone();
            removed.remove(&k);
            assert_eq!(updated, removed);
            let mut different_order = Map::empty();
            for q in (0..4).rev() {
                if let Some(v) = original.get(&q) {
                    different_order.insert(q, *v);
                }
            }
            assert_eq!(original, different_order);
        }
    }
}
