//! The lockfile: found proofs stored in `Locus.lock`.
//!
//! `locus check` writes the proofs it finds to `Locus.lock` in the directory
//! of the file it checked, one entry per obligation under one table per
//! file, and reads them back on the next run, so that a checked file is
//! checked again by the kernel alone. The file is untrusted input: an entry
//! is a hint, found by a key and handed to the kernel exactly as a freshly
//! found proof is, and it can cost a search when it is stale or wrong and
//! never make a false claim pass.
//!
//! An obligation is keyed by the text `text::print_key` gives it: the
//! context, entry by entry, and the claim, all as kernel terms with names
//! for declarations and positions for the context's own entries. The key is
//! the FNV-1a hash of that text, 64 bits, written as 16 hex digits. Nothing
//! of the source text enters it, so reformatting the source, commenting
//! it, and editing other functions leave every entry in use. The hash finds
//! an entry and carries no authority.
//!
//! The format, version 2, is TOML (`lock.rs`):
//!
//! ~~~toml
//! version = 2
//!
//! [[file]]
//! path = "lock.lc"
//!
//!   [[file.obligation]]
//!   key = "3e5398446530a8a5"
//!   at = "run 2"
//!   claim = "fn:within_limit($10.0)"
//!   steps = '''
//! t1 = fn:within_limit(#0.0)
//! s1 = transport(transport(h4, (#0 ==[struct:Lock] $11), refl($11)), t1, ...)
//! '''
//! ~~~
//!
//! `path` is relative to the lockfile's directory, with forward slashes;
//! the files are in path order. `key` finds the entry. `at`, the function
//! and the ordinal of the obligation within it, is for the reader and not
//! part of the key. `claim` is what the stored proof concludes, in the text
//! form, so that a stale entry can say what it proves and the obligation
//! can say what it wants. `steps` is the proof as named steps (`steps.rs`).
//! The obligations of a file are in the order the run met them, which is
//! the order of the items as they are elaborated, dependencies first and
//! otherwise as in the source, and of the obligations within each. An entry
//! nothing asked for in a run is dropped when the file is written, an
//! entry that was used is rewritten in the writer's own form, and the
//! entries of other files are kept as they were, so a run on an unchanged
//! source writes the same bytes. The writer owns the file: it carries no
//! comments and is rewritten whole.
//!
//! Version 1 wrote one file beside each source, `<source>.proofs`, with the
//! proofs as trees (`v1.rs`). A run that finds such a file for the source
//! it checks, and no entry for the source in the lockfile, reads it, uses
//! its entries as it would the lockfile's, and after a successful check
//! writes them into the lockfile and deletes it.
//!
//! The store is handed to the elaborator for one file by
//! `elab::elaborate_with_store`, which installs it in a thread-local slot
//! for the duration; the elaborator's hole solver looks each obligation up
//! there before searching and records what it finds. A slot that is empty
//! means no store: every obligation is searched and nothing is written,
//! which is how the tests that exercise the search itself run.

mod lock;
mod steps;
pub mod text;
pub mod v1;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

pub use lock::{FILE_NAME, FORMAT_VERSION, Lockfile, MAX_FILE};
pub use text::{Names, ParseError, PrintError};

/// The key of an obligation: the FNV-1a hash of its key text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key(u64);

impl Key {
    /// FNV-1a, 64 bits, over the bytes of the text. Stable across
    /// platforms and versions of the compiler, and dependency-free.
    pub fn of(text: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        Self(hash)
    }

    /// Sixteen hex digits, as the key is written.
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != 16 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        u64::from_str_radix(text, 16).ok().map(Self)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Where an obligation arose, for the reader of the file: the function and
/// the obligation's ordinal within it, from 1. Written as `at = "run 2"`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Label {
    pub function: String,
    pub ordinal: usize,
}

impl Label {
    /// Reads `function ordinal`; anything else is a label with that text
    /// as its function and no ordinal, since the label is only shown.
    pub fn parse(text: &str) -> Self {
        let text = text.trim();
        match text.rsplit_once(' ') {
            Some((function, ordinal)) if !function.is_empty() => Self {
                function: function.to_string(),
                ordinal: ordinal.parse().unwrap_or(0),
            },
            _ => Self {
                function: text.to_string(),
                ordinal: 0,
            },
        }
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.function, self.ordinal)
    }
}

/// One stored proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: Key,
    pub label: Label,
    /// What the proof concludes, in the text form; empty when unknown, as
    /// for an entry read from a version-1 file, until the entry is used.
    pub claim: String,
    /// The proof, as a block of steps.
    pub steps: String,
}

/// An entry as the store holds it.
#[derive(Clone, Debug)]
struct Held {
    entry: Entry,
    /// Asked for in this run: looked up and accepted, or found and recorded.
    used: bool,
    /// When the run first asked for it, from 0: the order entries are
    /// written in.
    sequence: Option<usize>,
}

/// What a lookup fixed about the obligation it was for, until the
/// obligation is accepted, refused, or recorded.
#[derive(Clone, Debug)]
struct Pending {
    label: Label,
    claim: String,
    sequence: usize,
}

/// What a run did with the store, for a report and for the tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Obligations a stored proof was accepted for.
    pub hits: usize,
    /// Obligations no entry was found for.
    pub misses: usize,
    /// Entries found but refused: they did not read as a proof, or the
    /// kernel rejected them against the current claim.
    pub stale: usize,
    /// Times the search was allowed to run.
    pub searches: usize,
    /// Proofs the search found that were recorded.
    pub recorded: usize,
    /// Proofs the search found that could not be written, because they
    /// mention something the text has no name for.
    pub unprintable: usize,
}

/// An obligation of this run that no stored proof met, in the order the
/// run met them: a miss, or a stale entry with what it concluded and what
/// the obligation wanted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    pub label: Label,
    /// For a stale entry: the claim it stored and the claim wanted, when
    /// they differ. An entry that did not read as a proof, or that
    /// claimed the right thing and was still rejected, has `None`.
    pub stale: Option<(String, String)>,
}

/// The proofs of one source file, and what the run does with them.
#[derive(Clone, Debug)]
pub struct ProofStore {
    entries: BTreeMap<Key, Held>,
    pending: HashMap<Key, Pending>,
    /// The claim of the obligation last keyed (`text::print_key`).
    claim: Option<String>,
    /// The canonical text of the proof last read (`text::parse_proof`).
    canonical: Option<String>,
    /// How many obligations the run has asked for, for the sequence.
    asked: usize,
    /// Never search: a miss is an error.
    locked: bool,
    /// Whether the search may run at all. Off, a miss is simply unsolved;
    /// this is the test hook that stands in for a weaker future search.
    search: bool,
    stats: Stats,
    misses: Vec<Miss>,
    /// The item obligations are being asked for, and how many it has asked.
    item: String,
    ordinal: usize,
    /// The names the last obligation was printed with, which name every
    /// declaration made before it.
    names: Option<Names>,
}

impl Default for ProofStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofStore {
    /// An empty store: every obligation misses and is searched.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            pending: HashMap::new(),
            claim: None,
            canonical: None,
            asked: 0,
            locked: false,
            search: true,
            stats: Stats::default(),
            misses: Vec::new(),
            item: String::new(),
            ordinal: 0,
            names: None,
        }
    }

    /// A store holding `entries`, as read from a file: none used yet. An
    /// entry whose key appears twice is kept once, the first.
    pub fn with_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
        let mut store = Self::new();
        for entry in entries {
            store.entries.entry(entry.key).or_insert(Held {
                entry,
                used: false,
                sequence: None,
            });
        }
        store
    }

    /// Never search: a miss is an error the elaborator reports.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Whether the search may run on a miss.
    pub fn searching(mut self, search: bool) -> Self {
        self.search = search;
        self
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// The obligations no stored proof met, in the order they were met.
    pub fn misses(&self) -> &[Miss] {
        &self.misses
    }

    /// The number of entries, read or recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every entry, read or recorded, in key order.
    pub fn entries(&self) -> Vec<&Entry> {
        self.entries.values().map(|held| &held.entry).collect()
    }

    /// The number of entries asked for in this run.
    pub fn used(&self) -> usize {
        self.entries.values().filter(|held| held.used).count()
    }

    /// The entries asked for in this run, in the order they were asked
    /// for: what is written.
    pub fn used_entries(&self) -> Vec<&Entry> {
        let mut used: Vec<&Held> = self.entries.values().filter(|held| held.used).collect();
        used.sort_by(|a, b| {
            a.sequence
                .cmp(&b.sequence)
                .then_with(|| a.entry.label.cmp(&b.entry.label))
                .then_with(|| a.entry.key.cmp(&b.entry.key))
        });
        used.into_iter().map(|held| &held.entry).collect()
    }

    /// The labels of the entries asked for, in the order they are written.
    pub fn used_labels(&self) -> Vec<Label> {
        self.used_entries()
            .into_iter()
            .map(|entry| entry.label.clone())
            .collect()
    }

    /// The next obligation of `item`: its label.
    fn label(&mut self, item: &str) -> Label {
        if self.item != item {
            self.item = item.to_string();
            self.ordinal = 0;
        }
        self.ordinal += 1;
        Label {
            function: item.to_string(),
            ordinal: self.ordinal,
        }
    }

    /// Keeps the claim of the obligation about to be looked up, which
    /// `text::print_key` printed.
    pub fn expect_claim(&mut self, claim: String) {
        self.claim = Some(claim);
    }

    /// Keeps the canonical text of the proof just read, which
    /// `text::parse_proof` printed; `None` when it could not be printed.
    pub fn read_as(&mut self, canonical: Option<String>) {
        self.canonical = canonical;
    }

    /// Version-1 sidecars have no recorded claim and predate layout-keying.
    /// Rekey only such candidates. This supplies a hint, not evidence: the
    /// ordinary parse/check/accept path still checks it against today's goal.
    pub(crate) fn rekey_legacy(&mut self, old: Key, new: Key) {
        if !self.entries.contains_key(&new)
            && self
                .entries
                .get(&old)
                .is_some_and(|h| h.entry.claim.is_empty())
            && let Some(mut held) = self.entries.remove(&old)
        {
            held.entry.key = new;
            self.entries.insert(new, held);
        }
    }

    /// The stored proof text of an obligation, if there is one. Whether it
    /// is accepted is for the caller to decide with `accept` or `refuse`;
    /// the ordinal of the obligation is consumed either way.
    pub fn lookup(&mut self, key: Key, item: &str) -> Option<String> {
        let label = self.label(item);
        let claim = self.claim.take().unwrap_or_default();
        self.canonical = None;
        let sequence = self.asked;
        self.asked += 1;
        self.pending.insert(
            key,
            Pending {
                label: label.clone(),
                claim,
                sequence,
            },
        );
        match self.entries.get_mut(&key) {
            Some(held) => {
                held.entry.label = label;
                if held.sequence.is_none() {
                    held.sequence = Some(sequence);
                }
                Some(held.entry.steps.clone())
            }
            None => {
                self.stats.misses += 1;
                self.misses.push(Miss { label, stale: None });
                None
            }
        }
    }

    /// The stored proof was accepted by the kernel: the entry stays, with
    /// the claim the obligation wanted, which it proved, and in the
    /// writer's own text.
    pub fn accept(&mut self, key: Key) {
        let pending = self.pending.remove(&key);
        let canonical = self.canonical.take();
        if let Some(held) = self.entries.get_mut(&key) {
            held.used = true;
            if let Some(pending) = pending
                && !pending.claim.is_empty()
            {
                held.entry.claim = pending.claim;
            }
            if let Some(canonical) = canonical {
                held.entry.steps = canonical;
            }
        }
        self.stats.hits += 1;
    }

    /// The stored proof was not accepted: it is dropped, and the obligation
    /// is a miss.
    pub fn refuse(&mut self, key: Key) {
        let pending = self.pending.get(&key);
        let dropped = self.entries.remove(&key);
        let label = pending
            .map(|pending| pending.label.clone())
            .or_else(|| dropped.as_ref().map(|held| held.entry.label.clone()))
            .unwrap_or_else(|| Label {
                function: self.item.clone(),
                ordinal: self.ordinal,
            });
        let stale = match (pending, dropped) {
            (Some(pending), Some(held))
                if !held.entry.claim.is_empty()
                    && !pending.claim.is_empty()
                    && held.entry.claim != pending.claim =>
            {
                Some((held.entry.claim, pending.claim.clone()))
            }
            _ => None,
        };
        self.misses.push(Miss { label, stale });
        self.canonical = None;
        self.stats.stale += 1;
        self.stats.misses += 1;
    }

    /// Whether the search may run for the obligation just looked up, which
    /// missed; counted.
    pub fn may_search(&mut self) -> bool {
        if self.locked || !self.search {
            return false;
        }
        self.stats.searches += 1;
        true
    }

    /// Records the proof the search found for the obligation just looked
    /// up, which missed, with the label and the claim the lookup fixed.
    pub fn record(&mut self, key: Key, steps: String) {
        let pending = self.pending.remove(&key);
        let (label, claim, sequence) = match pending {
            Some(pending) => (pending.label, pending.claim, Some(pending.sequence)),
            None => (
                Label {
                    function: self.item.clone(),
                    ordinal: self.ordinal,
                },
                String::new(),
                None,
            ),
        };
        self.entries.insert(
            key,
            Held {
                entry: Entry {
                    key,
                    label,
                    claim,
                    steps,
                },
                used: true,
                sequence,
            },
        );
        self.stats.recorded += 1;
    }

    /// The search found a proof the text cannot hold; counted.
    pub fn unprintable(&mut self) {
        self.stats.unprintable += 1;
    }

    /// Keeps the names the last obligation was printed with.
    pub fn set_names(&mut self, names: Names) {
        self.names = Some(names);
    }

    /// The names the last obligation of the run was printed with, for a
    /// tool that reads the entries back outside the elaborator: every
    /// declaration an entry can mention is named there.
    pub fn names(&self) -> Option<&Names> {
        self.names.as_ref()
    }

    /// A lockfile holding this store's used entries alone, under `path`:
    /// the text `Lockfile::put` would write for a lockfile that had
    /// nothing else.
    pub fn render(&self, path: &str) -> String {
        let mut lockfile = Lockfile::new();
        lockfile.put(path, self);
        lockfile.render()
    }
}

thread_local! {
    static CURRENT: RefCell<Option<ProofStore>> = const { RefCell::new(None) };
}

/// Runs `body` with `store` installed as the current store of this thread,
/// and returns the store afterwards, whatever `body` did with it.
pub fn with_store<R>(store: ProofStore, body: impl FnOnce() -> R) -> (R, ProofStore) {
    let previous = CURRENT.with(|current| current.replace(Some(store)));
    let result = body();
    let store = CURRENT.with(|current| current.replace(previous));
    (
        result,
        store.expect("the store installed for the body is still there"),
    )
}

/// Applies `f` to the current store, if one is installed.
pub fn with_current<R>(f: impl FnOnce(&mut ProofStore) -> R) -> Option<R> {
    CURRENT.with(|current| current.borrow_mut().as_mut().map(f))
}

/// Run an isolated compiler probe without reading, recording, or changing
/// the installed proof store. Restoration also happens during unwinding.
pub(crate) fn without_store<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(Option<ProofStore>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|current| {
                current.replace(self.0.take());
            });
        }
    }
    let _restore = Restore(CURRENT.with(|current| current.replace(None)));
    body()
}
