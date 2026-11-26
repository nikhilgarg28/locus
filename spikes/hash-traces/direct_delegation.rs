//! Rust-only probes of delegated standard hashing; not Locus verification.
#![feature(hasher_prefixfree_extras)]
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Bytes(Vec<u8>),
    U8(u8),
    Text(String),
    Length(usize),
}
#[derive(Default)]
struct DistinguishingHasher {
    tag: u64,
    calls: Vec<Call>,
}
impl Hasher for DistinguishingHasher {
    fn finish(&self) -> u64 {
        self.tag
    }
    fn write(&mut self, value: &[u8]) {
        self.tag = 1;
        self.calls.push(Call::Bytes(value.to_vec()));
    }
    fn write_u8(&mut self, value: u8) {
        self.tag = 2;
        self.calls.push(Call::U8(value));
    }
    fn write_str(&mut self, value: &str) {
        self.tag = 3;
        self.calls.push(Call::Text(value.into()));
    }
    fn write_length_prefix(&mut self, value: usize) {
        self.tag = 4;
        self.calls.push(Call::Length(value));
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    StdText(String),
}
// Runtime logging only for observing the experiment. Locus's log is ghost.
struct Sink<'a, H> {
    inner: &'a mut H,
    events: Vec<Event>,
}
impl<'a, H: Hasher> Sink<'a, H> {
    fn new(inner: &'a mut H) -> Self {
        Self {
            inner,
            events: vec![],
        }
    }
    fn string(&mut self, value: &String) {
        value.hash(self.inner);
        self.events.push(Event::StdText(value.clone()));
    }
    fn text(&mut self, value: &str) {
        value.hash(self.inner);
        self.events.push(Event::StdText(value.into()));
    }
}
// Deliberately no Hasher implementation for Sink.

#[derive(PartialEq, Eq)]
struct OwnedKey(String);
impl Hash for OwnedKey {
    fn hash<H: Hasher>(&self, h: &mut H) {
        Sink::new(h).string(&self.0);
    }
}
impl Borrow<str> for OwnedKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

#[test]
fn direct_delegation_preserves_override_and_borrowed_lookup() {
    let value = String::from("abc");
    let mut owned = DistinguishingHasher::default();
    let mut borrowed = DistinguishingHasher::default();
    let mut a = Sink::new(&mut owned);
    let mut b = Sink::new(&mut borrowed);
    a.string(&value);
    b.text(&value);
    assert_eq!(a.events, b.events); // One canonical semantic event.
    drop(a);
    drop(b);
    assert_eq!(owned.calls, vec![Call::Text(value)]);
    assert_eq!(owned.calls, borrowed.calls);
    let mut map: HashMap<OwnedKey, u32, BuildHasherDefault<DistinguishingHasher>> =
        HashMap::default();
    map.insert(OwnedKey("abc".into()), 17);
    assert_eq!(map.get(&OwnedKey("abc".into())), Some(&17));
    assert_eq!(map.get("abc"), Some(&17));
}

#[test]
fn standard_u8_slice_hash_is_not_a_loop_of_element_hashes() {
    let bytes = [1u8, 2];
    let mut standard = DistinguishingHasher::default();
    bytes.as_slice().hash(&mut standard);
    let mut looped = DistinguishingHasher::default();
    looped.write_length_prefix(bytes.len());
    for b in bytes {
        b.hash(&mut looped);
    }
    assert_eq!(
        standard.calls,
        vec![Call::Length(2), Call::Bytes(vec![1, 2])]
    );
    assert_eq!(
        looped.calls,
        vec![Call::Length(2), Call::U8(1), Call::U8(2)]
    );
    assert_ne!(standard.finish(), looped.finish());
}
