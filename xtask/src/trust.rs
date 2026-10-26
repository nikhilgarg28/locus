//! The trust model, `design/TRUST.md`: the entries in the tables under
//! `## Trusted components` and `## Assumptions`, each with a permanent ID in
//! its first column.

use std::collections::BTreeMap;

use crate::markdown;
use crate::repo::Repo;
use crate::report::Report;
use crate::spec;

pub const PATH: &str = "design/TRUST.md";

/// The sections that hold entries, and the prefix of their IDs.
const SECTIONS: &[(&str, &str)] = &[
    ("Trusted components", "trusted"),
    ("Assumptions", "assumed"),
];

pub struct Entry {
    pub id: String,
    pub line: usize,
    /// What the entry is about: its second cell.
    pub name: String,
    /// The numbers of the RFCs its last cell names.
    pub rfcs: Vec<String>,
}

pub fn check(repo: &Repo, report: &mut Report) -> Vec<Entry> {
    let Some(text) = repo.text(PATH) else {
        report.error(PATH, None, "the trust model is missing");
        return Vec::new();
    };
    let mut entries = Vec::new();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    let mut prefix = None;
    let mut header_seen = false;
    for line in markdown::lines(text) {
        if line.in_fence {
            continue;
        }
        if let Some(heading) = line.text.strip_prefix("## ") {
            prefix = SECTIONS
                .iter()
                .find(|(name, _)| *name == heading.trim())
                .map(|(_, prefix)| *prefix);
            header_seen = false;
            continue;
        }
        let row = line.text.trim();
        let Some(prefix) = prefix.filter(|_| row.starts_with('|')) else {
            continue;
        };
        let cells: Vec<&str> = row.trim_matches('|').split('|').map(str::trim).collect();
        if !header_seen {
            header_seen = true;
            if cells.first() != Some(&"ID") {
                report.error(
                    PATH,
                    Some(line.number),
                    "the first column of an entry table is `ID`",
                );
            }
            continue;
        }
        if cells
            .iter()
            .all(|c| c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        let id = cells[0].strip_prefix('`').and_then(|c| c.strip_suffix('`'));
        let Some(id) =
            id.filter(|id| spec::is_valid_id(id) && id.starts_with(&format!("{prefix}.")))
        else {
            report.error(
                PATH,
                Some(line.number),
                format!("an entry starts with its ID in backticks: `{prefix}.<name>`, in dotted kebab-case"),
            );
            continue;
        };
        if let Some(other) = ids.insert(id.to_string(), line.number) {
            report.error(
                PATH,
                Some(line.number),
                format!("`{id}` is already the ID of line {other}"),
            );
        }
        let last = cells.last().copied().unwrap_or_default();
        entries.push(Entry {
            id: id.to_string(),
            line: line.number,
            name: cells.get(1).copied().unwrap_or_default().to_string(),
            rfcs: rfc_numbers(last),
        });
    }
    entries
}

/// The four-digit numbers that follow `RFC ` in `text`.
fn rfc_numbers(text: &str) -> Vec<String> {
    text.match_indices("RFC ")
        .filter_map(|(at, _)| text.get(at + 4..at + 8))
        .filter(|n| n.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_entries_with_ids() {
        let text = "\
# Trust model

## Trusted components

| ID | Component | Added by |
|---|---|---|
| `trusted.kernel` | The kernel | [RFC 0002](rfcs/0002-x.md) |
| `assumed.wrong-section` | X | RFC 0001 |
| `trusted.kernel` | Again | RFC 0001, RFC 0003 |

## Assumptions

| Assumption | Added by |
|---|---|
| `assumed.lean` | RFC 0002 |

## Rules

| Not | Entries |
";
        let repo = Repo::from_files(&[(PATH, text)]);
        let mut report = Report::default();
        let entries = check(&repo, &mut report);
        assert_eq!(
            report.messages(),
            [
                "error: design/TRUST.md:8: an entry starts with its ID in backticks: `trusted.<name>`, in dotted kebab-case",
                "error: design/TRUST.md:9: `trusted.kernel` is already the ID of line 7",
                "error: design/TRUST.md:13: the first column of an entry table is `ID`",
            ]
        );
        let found: Vec<(&str, &str, Vec<&str>)> = entries
            .iter()
            .map(|e| {
                (
                    e.id.as_str(),
                    e.name.as_str(),
                    e.rfcs.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("trusted.kernel", "The kernel", vec!["0002"]),
                ("trusted.kernel", "Again", vec!["0001", "0003"]),
                ("assumed.lean", "RFC 0002", vec!["0002"]),
            ]
        );
    }
}
