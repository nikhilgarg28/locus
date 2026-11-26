//! Finite probes of native behavior; deliberately bad impls violate Rust's
//! documented library laws, not Rust memory safety. No Locus acceptance claim.
#![forbid(unsafe_code)]

use borrow_bridge_probe::*;
use std::borrow::Borrow;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

#[test]
fn standard_string_str_uses_actual_borrow_in_both_maps() {
    let mut hash = HashMap::new();
    let mut tree = BTreeMap::new();
    hash.insert("alice".to_owned(), 7);
    tree.insert("alice".to_owned(), 7);
    let independent_query = String::from("alice");
    assert_eq!(hash.get(independent_query.as_str()), Some(&7));
    assert_eq!(tree.get(independent_query.as_str()), Some(&7));
    let key = String::from("alice");
    assert_eq!(native_borrow::<String, str, StdText>(&key), "alice");
    assert_eq!(trace(&key), trace(key.as_str()));
}

#[test]
fn generated_newtype_provider_works_with_native_maps() {
    let key = Name("alice".into());
    assert_eq!(native_borrow::<Name, str, GeneratedName>(&key), "alice");
    assert_eq!(trace(&key), trace("alice"));
    let hash = HashMap::from([(key.clone(), 9)]);
    let tree = BTreeMap::from([(key, 9)]);
    assert_eq!(hash.get("alice"), Some(&9));
    assert_eq!(tree.get("alice"), Some(&9));
}

#[test]
fn reflexive_adapter_reuses_existing_blanket_sized_and_unsized() {
    let name = Name("bob".into());
    assert!(std::ptr::eq(
        native_borrow::<Name, Name, Identity>(&name),
        &name
    ));
    assert_eq!(native_borrow::<str, str, Identity>("bob"), "bob");
    let slice: &[u32] = &[1, 2];
    assert_eq!(native_borrow::<[u32], [u32], Identity>(slice), slice);
    assert_eq!(
        <Name as BorrowOperation<Name>>::borrow_verified(&name),
        &name
    );
}

#[test]
fn concrete_generic_wrapper_supports_unsized_targets() {
    let text = Owned::<str>(Box::from("abc"));
    let slice = Owned::<[u8]>(Box::from([1, 2]));
    assert_eq!(<Owned<str> as Borrow<str>>::borrow(&text), "abc");
    assert_eq!(<Owned<[u8]> as Borrow<[u8]>>::borrow(&slice), &[1, 2]);
}

#[test]
fn vec_slice_query_needs_no_owned_query_allocation() {
    let hash = HashMap::from([(vec![1u8, 2, 3], 4)]);
    let tree = BTreeMap::from([(vec![1u8, 2, 3], 4)]);
    assert_eq!(hash.get(&[1u8, 2, 3][..]), Some(&4));
    assert_eq!(tree.get(&[1u8, 2, 3][..]), Some(&4));
    assert_eq!(trace(&vec![1u8, 2, 3]), trace(&[1u8, 2, 3][..]));
    let v = vec![1u8, 2, 3];
    assert_eq!(
        <Vec<u8> as BorrowOperation<[u8]>>::borrow_verified(&v),
        &v[..]
    );
}

#[derive(Debug)]
struct Split {
    intended: String,
    actual: String,
}
impl BorrowOperation<str> for Split {
    fn borrow_verified(&self) -> &str {
        &self.intended
    }
}
impl Borrow<str> for Split {
    fn borrow(&self) -> &str {
        &self.actual
    }
}
impl PartialEq for Split {
    fn eq(&self, rhs: &Self) -> bool {
        self.intended == rhs.intended
    }
}
impl Eq for Split {}
impl PartialOrd for Split {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Split {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.intended.cmp(&rhs.intended)
    }
}

#[test]
fn correct_operation_plus_borrow_bound_does_not_bind_actual_impl() {
    fn naive<K: BorrowOperation<str> + Borrow<str>>(k: &K) -> &str {
        k.borrow_verified()
    }
    let key = Split {
        intended: "alice".into(),
        actual: "mallory".into(),
    };
    assert_eq!(naive(&key), "alice");
    assert_eq!(<Split as Borrow<str>>::borrow(&key), "mallory");
    let tree = BTreeMap::from([(key, 7)]);
    assert_eq!(tree.get("alice"), None);
    assert_eq!(tree.get("mallory"), Some(&7));
}

#[derive(Debug)]
struct Folded(String);
impl PartialEq for Folded {
    fn eq(&self, rhs: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&rhs.0)
    }
}
impl Eq for Folded {}
impl Hash for Folded {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.to_ascii_lowercase().hash(h);
    }
}
impl Borrow<str> for Folded {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[test]
fn case_insensitive_key_cannot_certify_case_sensitive_str_lookup() {
    let a = Folded("ALICE".into());
    let b = Folded("alice".into());
    assert_eq!(a, b);
    assert_ne!(
        <Folded as Borrow<str>>::borrow(&a),
        <Folded as Borrow<str>>::borrow(&b)
    );
    assert_eq!(trace(&a), trace("alice"));
    assert_ne!(trace(&a), trace("ALICE"));
    // Even when this query's hash agrees, the actual str equality rejects it.
    let map = HashMap::from([(a, 8)]);
    assert_eq!(map.get("alice"), None);
}

#[test]
fn canonical_projection_can_use_str_but_queries_remain_case_sensitive() {
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Canonical(String);
    impl Canonical {
        fn new(s: &str) -> Self {
            Self(s.to_ascii_lowercase())
        }
    }
    impl Borrow<str> for Canonical {
        fn borrow(&self) -> &str {
            &self.0
        }
    }
    // The stored projection is canonical. Borrow does not normalize a query.
    let hash = HashMap::from([(Canonical::new("ALICE"), 3)]);
    let tree = BTreeMap::from([(Canonical::new("ALICE"), 3)]);
    assert_eq!(hash.get("alice"), Some(&3));
    assert_eq!(tree.get("alice"), Some(&3));
    assert_eq!(hash.get("ALICE"), None);
    assert_eq!(tree.get("ALICE"), None);
    assert_eq!(trace(&Canonical::new("ALICE")), trace("alice"));
}

#[derive(Debug, PartialEq, Eq)]
struct Salted(String);
impl Borrow<str> for Salted {
    fn borrow(&self) -> &str {
        &self.0
    }
}
impl Hash for Salted {
    fn hash<H: Hasher>(&self, h: &mut H) {
        19u32.hash(h);
        self.0.hash(h);
    }
}

#[test]
fn equality_and_correct_projection_do_not_imply_hash_compatibility() {
    let key = Salted("alice".into());
    assert_eq!(<Salted as Borrow<str>>::borrow(&key), "alice");
    assert_ne!(trace(&key), trace("alice"));
}

#[derive(Debug, PartialEq, Eq)]
struct Reversed(String);
impl PartialOrd for Reversed {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Reversed {
    fn cmp(&self, rhs: &Self) -> Ordering {
        rhs.0.cmp(&self.0)
    }
}
impl Borrow<str> for Reversed {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[test]
fn preserving_only_equal_does_not_preserve_tree_search_order() {
    let a = Reversed("a".into());
    let c = Reversed("c".into());
    assert_eq!(a.cmp(&a), "a".cmp("a"));
    assert_ne!(a.cmp(&c), "a".cmp("c"));
    let map = BTreeMap::from([(a, 1), (Reversed("b".into()), 2), (c, 3)]);
    assert_eq!(map.get("a"), None);
}

#[derive(Debug)]
struct ById {
    id: u32,
    label: String,
}
impl PartialEq for ById {
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl Eq for ById {}
impl PartialOrd for ById {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for ById {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.id.cmp(&rhs.id)
    }
}
impl Hash for ById {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h);
    }
}
impl Borrow<u32> for ById {
    fn borrow(&self) -> &u32 {
        &self.id
    }
}

#[test]
fn projection_may_forget_label_and_preserve_selected_equivalence() {
    let a = ById {
        id: 7,
        label: "Alice".into(),
    };
    let b = ById {
        id: 7,
        label: "Alicia".into(),
    };
    assert_eq!(a, b);
    assert_ne!(a.label, b.label);
    assert_eq!(trace(&a), trace(&7u32));
    let mut tree = BTreeMap::from([(a, 10)]);
    tree.insert(b, 20);
    let (key, value) = tree.get_key_value(&7u32).unwrap();
    assert_eq!(key.label, "Alice");
    assert_eq!(*value, 20);
}

#[test]
fn queries_outside_projection_image_are_ordinary_misses() {
    // This fixture's finite key domain contains only even IDs; odd queries
    // still make sense. The laws must cover all valid queries, not only p(k).
    let map = BTreeMap::from([(
        ById {
            id: 2,
            label: "two".into(),
        },
        9,
    )]);
    assert_eq!(map.get(&2u32), Some(&9));
    assert_eq!(map.get(&3u32), None);
}

#[test]
fn hidden_mutable_hash_state_cannot_factor_through_unit_snapshot() {
    struct Hidden(Cell<u32>);
    impl Hash for Hidden {
        fn hash<H: Hasher>(&self, h: &mut H) {
            self.0.get().hash(h);
        }
    }
    let key = Hidden(Cell::new(1));
    let before_snapshot = (); // Proposed structural snapshot of the Cell.
    let before_trace = trace(&key);
    key.0.set(2);
    assert_eq!(before_snapshot, ());
    assert_ne!(before_trace, trace(&key));
}

#[test]
fn panic_produces_no_successful_borrow_result() {
    struct Panic;
    impl BorrowOperation<str> for Panic {
        fn borrow_verified(&self) -> &str {
            panic!("no normal return")
        }
    }
    let result = std::panic::catch_unwind(|| {
        let value = <Panic as BorrowOperation<str>>::borrow_verified(&Panic);
        assert_eq!(value, "never returned");
    });
    assert!(result.is_err());
}
