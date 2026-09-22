//! The proofs file: found proofs stored beside the source.
//!
//! `locus check` writes the proofs it finds to `<source>.proofs`, one entry
//! per obligation, and reads them back on the next run, so that a checked
//! file is checked again by the kernel alone. The file is untrusted input:
//! an entry is a hint, found by a key and handed to the kernel exactly as a
//! freshly found proof is, and it can cost a search when it is stale or
//! wrong and never make a false claim pass.
//!
//! An obligation is keyed by the text `text::print_key` gives it: the
//! context, entry by entry, and the claim, all as kernel terms with names
//! for declarations and positions for the context's own entries. The key is
//! the FNV-1a hash of that text, 64 bits, written as 16 hex digits. Nothing
//! of the source text enters it, so reformatting the source, commenting
//! it, and editing other functions leave every entry in use. The hash finds
//! an entry and carries no authority.
//!
//! The format, version 1:
//!
//! ~~~text
//! locus-proofs 1
//!
//! obligation 9f86d081884c7d65 step 1
//! implies_elim(h0, refl($1))
//!
//! obligation ... run 1
//! ...
//! ~~~
//!
//! The header names the version; a file with another version is refused
//! whole. Each entry is a key line, `obligation <key> <function> <ordinal>`,
//! where the function and the ordinal of the obligation within it are for
//! the reader and not part of the key, and one line holding the proof in the
//! text form of `text.rs`. Entries are written in the order of their
//! labels, and an entry nothing asked for in a run is dropped when the file
//! is written, so a run on an unchanged source writes the same bytes.
//!
//! The store is handed to the elaborator for one file by
//! `elab::elaborate_with_store`, which installs it in a thread-local slot
//! for the duration; the elaborator's hole solver looks each obligation up
//! there before searching and records what it finds. A slot that is empty
//! means no store: every obligation is searched and nothing is written,
//! which is how the tests that exercise the search itself run.

pub mod text;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

pub use text::{Names, ParseError, PrintError};

/// The version of the file format, in its header line.
pub const FORMAT_VERSION: u32 = 1;

const HEADER: &str = "locus-proofs";

/// The most bytes a proofs file may be.
pub const MAX_FILE: usize = 64 << 20;

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

    fn parse(text: &str) -> Option<Self> {
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
/// the obligation's ordinal within it, from 1.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Label {
    pub function: String,
    pub ordinal: usize,
}

#[derive(Clone, Debug)]
struct Entry {
    label: Label,
    proof: String,
    /// Asked for in this run: looked up and accepted, or found and recorded.
    used: bool,
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

/// The proofs of one source file.
#[derive(Clone, Debug)]
pub struct ProofStore {
    entries: BTreeMap<Key, Entry>,
    /// The file as it was read, to know whether writing would change it.
    read: Option<String>,
    /// Never search: a miss is an error.
    locked: bool,
    /// Whether the search may run at all. Off, a miss is simply unsolved;
    /// this is the test hook that stands in for a weaker future search.
    search: bool,
    stats: Stats,
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
            read: None,
            locked: false,
            search: true,
            stats: Stats::default(),
            item: String::new(),
            ordinal: 0,
            names: None,
        }
    }

    /// Reads a file. A header of another version, or none, refuses the
    /// whole file; a malformed entry is skipped with a warning, and a
    /// truncated last entry is dropped with one. What is returned is a
    /// store and the warnings, never a panic, on any text.
    pub fn parse(file: &str) -> Result<(Self, Vec<String>), String> {
        if file.len() > MAX_FILE {
            return Err(format!("the proofs file is larger than {MAX_FILE} bytes"));
        }
        let mut store = Self::new();
        let mut warnings = Vec::new();
        let mut lines = file.lines().enumerate().peekable();
        let header = loop {
            match lines.next() {
                Some((_, line)) if line.trim().is_empty() => continue,
                Some((_, line)) => break line,
                None => return Err("the proofs file is empty".into()),
            }
        };
        match header.split_whitespace().collect::<Vec<_>>().as_slice() {
            [HEADER, version] if version.parse::<u32>() == Ok(FORMAT_VERSION) => {}
            [HEADER, version] => {
                return Err(format!(
                    "the proofs file is version {version}, and this locus reads version {FORMAT_VERSION}"
                ));
            }
            _ => return Err("the proofs file does not start with `locus-proofs 1`".into()),
        }
        while let Some((number, line)) = lines.next() {
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split_whitespace().collect();
            let key_line = match fields.as_slice() {
                ["obligation", key, function, ordinal] => Key::parse(key).and_then(|key| {
                    Some((
                        key,
                        Label {
                            function: (*function).to_string(),
                            ordinal: ordinal.parse().ok()?,
                        },
                    ))
                }),
                _ => None,
            };
            let Some((key, label)) = key_line else {
                warnings.push(format!(
                    "line {}: not an obligation line; skipped",
                    number + 1
                ));
                continue;
            };
            let Some((_, proof)) = lines.next_if(|(_, next)| !next.trim().is_empty()) else {
                warnings.push(format!(
                    "line {}: the obligation has no proof; the file may be truncated",
                    number + 1
                ));
                continue;
            };
            if store.entries.contains_key(&key) {
                warnings.push(format!(
                    "line {}: obligation {key} appears twice; the first is kept",
                    number + 1
                ));
                continue;
            }
            store.entries.insert(
                key,
                Entry {
                    label,
                    proof: proof.trim().to_string(),
                    used: false,
                },
            );
        }
        store.read = Some(file.to_string());
        Ok((store, warnings))
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

    /// The number of entries, read or recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of entries asked for in this run.
    pub fn used(&self) -> usize {
        self.entries.values().filter(|entry| entry.used).count()
    }

    /// The labels of the entries asked for, in the order they are written.
    pub fn used_labels(&self) -> Vec<Label> {
        let mut labels: Vec<Label> = self
            .entries
            .values()
            .filter(|entry| entry.used)
            .map(|entry| entry.label.clone())
            .collect();
        labels.sort();
        labels
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

    /// The stored proof text of an obligation, if there is one. Whether it
    /// is accepted is for the caller to decide with `accept` or `refuse`;
    /// the ordinal of the obligation is consumed either way.
    pub fn lookup(&mut self, key: Key, item: &str) -> Option<String> {
        let label = self.label(item);
        match self.entries.get_mut(&key) {
            Some(entry) => {
                entry.label = label;
                Some(entry.proof.clone())
            }
            None => {
                self.stats.misses += 1;
                None
            }
        }
    }

    /// The stored proof was accepted by the kernel: the entry stays.
    pub fn accept(&mut self, key: Key) {
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.used = true;
        }
        self.stats.hits += 1;
    }

    /// The stored proof was not accepted: it is dropped, and the obligation
    /// is a miss.
    pub fn refuse(&mut self, key: Key) {
        self.entries.remove(&key);
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
    /// up, which missed. The label is the one the lookup gave it.
    pub fn record(&mut self, key: Key, proof: String) {
        let label = Label {
            function: self.item.clone(),
            ordinal: self.ordinal,
        };
        self.entries.insert(
            key,
            Entry {
                label,
                proof,
                used: true,
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

    /// The file: the entries asked for in this run, in the order of their
    /// labels, and nothing else.
    pub fn render(&self) -> String {
        let mut entries: Vec<(&Key, &Entry)> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.used)
            .collect();
        entries.sort_by(|(key, a), (other, b)| a.label.cmp(&b.label).then(key.cmp(other)));
        let mut out = format!("{HEADER} {FORMAT_VERSION}\n");
        for (key, entry) in entries {
            out.push('\n');
            out.push_str(&format!(
                "obligation {key} {} {}\n{}\n",
                entry.label.function, entry.label.ordinal, entry.proof
            ));
        }
        out
    }

    /// The file to write, when writing would change it: `None` when the
    /// rendering is what was read, or when nothing was read and there is
    /// nothing to write.
    pub fn changed(&self) -> Option<String> {
        let rendered = self.render();
        match &self.read {
            Some(read) if *read == rendered => None,
            Some(_) => Some(rendered),
            None if self.used() == 0 => None,
            None => Some(rendered),
        }
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
