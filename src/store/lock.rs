//! `Locus.lock` as a file: reading it with the `toml` crate, and writing it
//! by hand, so that the layout is fixed and two machines write the same
//! bytes.
//!
//! The reader is total: any text gives a lockfile and warnings, or one
//! reason the whole file is refused (it is not TOML, it is too large, or it
//! is another version). An entry that lacks a field, or whose key is not
//! sixteen hex digits, is skipped with a warning; a file table without a
//! path is skipped; the first of two tables for one path, and the first of
//! two entries for one key, is kept. The `toml` parser bounds its own
//! recursion (80 levels in the pinned toml 1.1.6 dependency); exceeding
//! that dependency limit returns a TOML parse error.

use std::fmt::Write;

use super::{Entry, Key, Label, ProofStore};

/// The version of the file format, its first line.
pub const FORMAT_VERSION: i64 = 2;

/// The name of the file, in the directory of the source checked.
pub const FILE_NAME: &str = "Locus.lock";

/// The most bytes a lockfile may be.
pub use crate::limits::MAX_PROOF_FILE_BYTES as MAX_FILE;

/// The entries of one source file, as the lockfile holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
struct File {
    path: String,
    entries: Vec<Entry>,
}

/// A lockfile: the entries of every file, in path order.
#[derive(Clone, Debug, Default)]
pub struct Lockfile {
    files: Vec<File>,
    /// The file as it was read, to know whether writing would change it.
    read: Option<String>,
}

impl Lockfile {
    /// An empty lockfile.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a lockfile: the entries it holds and the warnings about what
    /// it holds that was skipped, or why it is refused whole.
    pub fn parse(text: &str) -> Result<(Self, Vec<String>), String> {
        if text.len() > MAX_FILE {
            return Err(format!(
                "MAX_PROOF_FILE_BYTES limit of {MAX_FILE} was exceeded"
            ));
        }
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| {
            let line = error
                .span()
                .map(|span| text[..span.start.min(text.len())].matches('\n').count() + 1);
            match line {
                Some(line) => format!("not a TOML lockfile: line {line}: {}", error.message()),
                None => format!("not a TOML lockfile: {}", error.message()),
            }
        })?;
        match table.get("version") {
            Some(toml::Value::Integer(FORMAT_VERSION)) => {}
            Some(toml::Value::Integer(version)) => {
                return Err(format!(
                    "the lockfile is version {version}, and this locus reads version {FORMAT_VERSION}"
                ));
            }
            Some(_) => return Err("the lockfile's version is not a number".into()),
            None => return Err("the lockfile has no `version`".into()),
        }
        let mut lockfile = Self::new();
        let mut warnings = Vec::new();
        let files = match table.get("file") {
            None => &[][..],
            Some(toml::Value::Array(files)) => files.as_slice(),
            Some(_) => {
                warnings.push("`file` is not an array of tables; no entry is read".into());
                &[][..]
            }
        };
        for (index, file) in files.iter().enumerate() {
            let Some(file) = file.as_table() else {
                warnings.push(format!("file {}: not a table; skipped", index + 1));
                continue;
            };
            let Some(path) = file.get("path").and_then(toml::Value::as_str) else {
                warnings.push(format!("file {}: no `path`; skipped", index + 1));
                continue;
            };
            if lockfile.has(path) {
                warnings.push(format!("`{path}` appears twice; the first is kept"));
                continue;
            }
            let mut entries: Vec<Entry> = Vec::new();
            let obligations = match file.get("obligation") {
                None => &[][..],
                Some(toml::Value::Array(obligations)) => obligations.as_slice(),
                Some(_) => {
                    warnings.push(format!(
                        "`{path}`: `obligation` is not an array of tables; no entry is read"
                    ));
                    &[][..]
                }
            };
            for (index, obligation) in obligations.iter().enumerate() {
                let mut skip = |why: &str| {
                    warnings.push(format!(
                        "`{path}`, obligation {}: {why}; skipped",
                        index + 1
                    ));
                };
                let Some(obligation) = obligation.as_table() else {
                    skip("not a table");
                    continue;
                };
                let field = |name: &str| obligation.get(name).and_then(toml::Value::as_str);
                let Some(key) = field("key") else {
                    skip("no `key`");
                    continue;
                };
                let Some(key) = Key::parse(key) else {
                    skip("the key is not sixteen hex digits");
                    continue;
                };
                let Some(steps) = field("steps") else {
                    skip("no `steps`");
                    continue;
                };
                if entries.iter().any(|entry| entry.key == key) {
                    skip("the key appears twice; the first is kept");
                    continue;
                }
                entries.push(Entry {
                    key,
                    label: Label::parse(field("at").unwrap_or("")),
                    claim: field("claim").unwrap_or("").trim().to_string(),
                    steps: steps.trim_end_matches('\n').to_string(),
                });
            }
            lockfile.insert(path, entries);
        }
        lockfile.read = Some(text.to_string());
        Ok((lockfile, warnings))
    }

    /// Whether the lockfile has a table for `path`.
    pub fn has(&self, path: &str) -> bool {
        self.files.iter().any(|file| file.path == path)
    }

    /// The paths of the files it has, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(|file| file.path.as_str())
    }

    /// The number of entries under `path`.
    pub fn entries(&self, path: &str) -> usize {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map_or(0, |file| file.entries.len())
    }

    /// Whether the lockfile holds nothing.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// `entries` as the table of `path`, replacing any it had.
    pub fn insert(&mut self, path: &str, entries: Vec<Entry>) {
        self.files.retain(|file| file.path != path);
        let at = self
            .files
            .iter()
            .position(|file| file.path.as_str() > path)
            .unwrap_or(self.files.len());
        self.files.insert(
            at,
            File {
                path: path.to_string(),
                entries,
            },
        );
    }

    /// The entries of `path`, as a store to check the file with; they are
    /// taken out of the lockfile until `put` puts them back.
    pub fn take(&mut self, path: &str) -> ProofStore {
        let entries = match self.files.iter().position(|file| file.path == path) {
            Some(at) => self.files.remove(at).entries,
            None => Vec::new(),
        };
        ProofStore::with_entries(entries)
    }

    /// The used entries of `store` under `path`, replacing whatever the
    /// lockfile had for it; a file with no entries has no table.
    pub fn put(&mut self, path: &str, store: &ProofStore) {
        let entries: Vec<Entry> = store.used_entries().into_iter().cloned().collect();
        if entries.is_empty() {
            self.files.retain(|file| file.path != path);
        } else {
            self.insert(path, entries);
        }
    }

    /// The file.
    pub fn render(&self) -> String {
        let mut out = format!("version = {FORMAT_VERSION}\n");
        for file in &self.files {
            out.push_str("\n[[file]]\npath = ");
            basic_string(&mut out, &file.path);
            out.push('\n');
            for entry in &file.entries {
                out.push_str("\n  [[file.obligation]]\n  key = \"");
                let _ = write!(out, "{}", entry.key);
                out.push_str("\"\n  at = ");
                basic_string(&mut out, &entry.label.to_string());
                out.push_str("\n  claim = ");
                basic_string(&mut out, &entry.claim);
                out.push_str("\n  steps = ");
                block_string(&mut out, &entry.steps);
                out.push('\n');
            }
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
            None if self.files.is_empty() => None,
            None => Some(rendered),
        }
    }
}

/// A TOML basic string, `"..."`, with whatever needs escaping escaped.
fn basic_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The steps as a multi-line literal string, `'''` on lines of its own
/// around the block, which is how the block reads best; a block the
/// literal form cannot hold, because it contains `'''` or a control
/// character, is written as a basic string instead.
fn block_string(out: &mut String, steps: &str) {
    let literal = !steps.contains("'''")
        && steps
            .chars()
            .all(|c| c == '\n' || c == '\t' || !c.is_control());
    if literal {
        out.push_str("'''\n");
        out.push_str(steps);
        out.push_str("\n'''");
    } else {
        basic_string(out, steps);
    }
}
