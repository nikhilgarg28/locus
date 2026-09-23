//! Version 1 of the proofs file, `<source>.proofs`, read for migration
//! only. The format:
//!
//! ~~~text
//! locus-proofs 1
//!
//! obligation 9f86d081884c7d65 step 1
//! implies_elim(h0, refl($1))
//! ~~~
//!
//! The header names the version. Each entry is a key line, `obligation
//! <key> <function> <ordinal>`, and one line holding the proof as a tree
//! in the text form, which `text::parse_proof` still reads. An entry read
//! from here has no claim; it gets one when it is used, and is written to
//! the lockfile in the writer's own form then.

use super::{Entry, Key, Label};

const HEADER: &str = "locus-proofs";
const VERSION: u32 = 1;

/// Reads a version-1 file. A header of another version, or none, refuses
/// the whole file; a malformed entry is skipped with a warning, and a
/// truncated last entry is dropped with one. What is returned is the
/// entries and the warnings, never a panic, on any text.
pub fn parse(file: &str) -> Result<(Vec<Entry>, Vec<String>), String> {
    if file.len() > super::MAX_FILE {
        return Err(format!(
            "MAX_PROOF_FILE_BYTES limit of {} was exceeded",
            super::MAX_FILE
        ));
    }
    let mut entries: Vec<Entry> = Vec::new();
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
        [HEADER, version] if version.parse::<u32>() == Ok(VERSION) => {}
        [HEADER, version] => {
            return Err(format!(
                "the proofs file is version {version}, and the migration reads version {VERSION}"
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
        if entries.iter().any(|entry| entry.key == key) {
            warnings.push(format!(
                "line {}: obligation {key} appears twice; the first is kept",
                number + 1
            ));
            continue;
        }
        entries.push(Entry {
            key,
            label,
            claim: String::new(),
            steps: proof.trim().to_string(),
        });
    }
    Ok((entries, warnings))
}
