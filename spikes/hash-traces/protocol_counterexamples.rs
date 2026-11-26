//! Concrete counterexamples for the proposed HashSink review. Rust only.
#![feature(hasher_prefixfree_extras)]
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};

#[derive(Default)]
struct DistinguishingHasher {
    tag: u64,
    calls: Vec<String>,
}
impl Hasher for DistinguishingHasher {
    fn finish(&self) -> u64 {
        self.tag
    }
    fn write(&mut self, bytes: &[u8]) {
        self.tag = 0x8000_0000_0000_0000;
        self.calls.push(format!("Bytes({bytes:?})"));
    }
    fn write_u8(&mut self, value: u8) {
        self.tag = 0x8000_0000_0000_0000;
        self.calls.push(format!("U8({value})"));
    }
    fn write_str(&mut self, value: &str) {
        self.tag = 0x4000_0000_0000_0000;
        self.calls.push(format!("Str({value:?})"));
    }
    fn write_length_prefix(&mut self, value: usize) {
        self.tag = 0x2000_0000_0000_0000;
        self.calls.push(format!("Length({value})"));
    }
}

struct StableOnlySink<'a, H: Hasher>(&'a mut H);
macro_rules! forward_stable {
    ($($name:ident($ty:ty)),* $(,)?) => {$(
        fn $name(&mut self, value: $ty) { self.0.$name(value); }
    )*};
}
impl<H: Hasher> Hasher for StableOnlySink<'_, H> {
    fn finish(&self) -> u64 {
        self.0.finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes);
    }
    forward_stable!(
        write_u8(u8),
        write_u16(u16),
        write_u32(u32),
        write_u64(u64),
        write_u128(u128),
        write_usize(usize),
        write_i8(i8),
        write_i16(i16),
        write_i32(i32),
        write_i64(i64),
        write_i128(i128),
        write_isize(isize)
    );
    // The proposed unstable-method fallback: both methods retain defaults.
}

#[test]
fn stable_only_sink_changes_string_hash_protocol() {
    let mut direct = DistinguishingHasher::default();
    let mut wrapped = DistinguishingHasher::default();
    "abc".hash(&mut direct);
    "abc".hash(&mut StableOnlySink(&mut wrapped));
    println!("direct {:?}; wrapped {:?}", direct.calls, wrapped.calls);
    assert_eq!(direct.calls, vec!["Str(\"abc\")"]);
    assert_eq!(wrapped.calls, vec!["Bytes([97, 98, 99])", "U8(255)"]);
    assert_ne!(direct.finish(), wrapped.finish());
}

#[derive(PartialEq, Eq)]
struct GeneratedKey(String);
impl Hash for GeneratedKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Simulates a new generated Hash that calls a standard-type checked
        // adapter implemented by passing the standard Hash through this sink.
        self.0.hash(&mut StableOnlySink(state));
    }
}
impl Borrow<str> for GeneratedKey {
    // Deliberately violates Borrow's hash-agreement requirement. Its contents
    // and equality agree; encoding disagreement is what this test demonstrates.
    fn borrow(&self) -> &str {
        &self.0
    }
}
#[test]
fn equal_borrowed_query_can_miss_when_routes_differ() {
    let mut map: HashMap<GeneratedKey, u32, BuildHasherDefault<DistinguishingHasher>> =
        HashMap::default();
    map.insert(GeneratedKey("abc".into()), 17);
    assert_eq!(map.get(&GeneratedKey("abc".into())), Some(&17));
    assert_eq!(map.get("abc"), None);
}

#[derive(Default)]
struct CallCounter(u64);
impl Hasher for CallCounter {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        self.0 += 1;
    }
}
#[test]
fn write_chunk_boundaries_matter() {
    let mut first = CallCounter::default();
    first.write(&[1]);
    first.write(&[2]);
    let mut second = CallCounter::default();
    second.write(&[1, 2]);
    assert_ne!(first.finish(), second.finish());
}

#[test]
fn hasher_implementation_cannot_hide_finish() {
    let mut h = DistinguishingHasher::default();
    let sink = StableOnlySink(&mut h);
    // No inherent finish method is needed. A publicly implemented trait makes
    // this interface available to every client that can name the trait.
    assert_eq!(Hasher::finish(&sink), 0);
}

#[derive(Default)]
struct MutateThenPanic {
    calls: u32,
}
impl Hasher for MutateThenPanic {
    fn finish(&self) -> u64 {
        u64::from(self.calls)
    }
    fn write(&mut self, _: &[u8]) {
        self.calls += 1;
        panic!("after mutation");
    }
}
struct Recorder<'a, H: Hasher> {
    inner: &'a mut H,
    events: Vec<Vec<u8>>,
}
impl<H: Hasher> Recorder<'_, H> {
    fn write(&mut self, bytes: &[u8]) {
        self.inner.write(bytes);
        self.events.push(bytes.to_vec());
    }
}
#[test]
fn normal_return_trace_update_needs_an_unwind_policy() {
    let mut h = MutateThenPanic::default();
    let mut sink = Recorder {
        inner: &mut h,
        events: vec![],
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink.write(&[1])));
    assert!(result.is_err());
    assert!(sink.events.is_empty());
    assert_eq!(sink.inner.finish(), 1);
    // This naive recorder lost correspondence. A proposal needs invalidation,
    // inaccessible failed sinks, or some other explicit recovery semantics.
}

struct OverridesHashOne;
impl BuildHasher for OverridesHashOne {
    type Hasher = CallCounter;
    fn build_hasher(&self) -> CallCounter {
        CallCounter::default()
    }
    fn hash_one<T: Hash>(&self, _: T) -> u64 {
        99
    }
}
#[test]
fn hash_one_override_needs_its_own_bridge_if_used() {
    let b = OverridesHashOne;
    let mut h = b.build_hasher();
    1u64.hash(&mut h);
    assert_eq!(h.finish(), 1);
    assert_eq!(b.hash_one(1u64), 99);
}

#[derive(PartialEq, Eq)]
struct EqualKey(u8);
impl Hash for EqualKey {
    fn hash<H: Hasher>(&self, h: &mut H) {
        h.write_u8(0);
    }
    fn hash_slice<H: Hasher>(keys: &[Self], h: &mut H) {
        // Intentionally violates batch hashing expectations despite a lawful
        // individual hash: even empty/equal slices can consult external state.
        static CALLS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
        h.write_u8(CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        let _ = keys;
    }
}
#[derive(Default)]
struct LastByte(u64);
impl Hasher for LastByte {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, b: &[u8]) {
        if let Some(x) = b.last() {
            self.0 = u64::from(*x);
        }
    }
    fn write_u8(&mut self, x: u8) {
        self.0 = u64::from(x);
    }
}
#[test]
fn individual_hash_law_does_not_certify_an_unchecked_hash_slice_override() {
    let keys = [EqualKey(7)];
    let mut a = LastByte::default();
    let mut b = LastByte::default();
    keys.as_slice().hash(&mut a);
    keys.as_slice().hash(&mut b);
    assert_ne!(a.finish(), b.finish());
}
