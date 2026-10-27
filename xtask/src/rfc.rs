//! RFCs: `design/rfcs/NNNN-slug.md`, each with a title and a status line.

use std::collections::BTreeMap;

use crate::litmus::is_kebab;
use crate::markdown;
use crate::repo::Repo;
use crate::report::Report;
use crate::spec;

pub const TEMPLATE: &str = "design/rfcs/0000-template.md";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Draft,
    Proposed,
    Accepted,
    Rejected,
    Superseded,
}

impl Status {
    fn parse(s: &str) -> Option<Status> {
        match s {
            "draft" => Some(Status::Draft),
            "proposed" => Some(Status::Proposed),
            "accepted" => Some(Status::Accepted),
            "rejected" => Some(Status::Rejected),
            "superseded" => Some(Status::Superseded),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Proposed => "proposed",
            Status::Accepted => "accepted",
            Status::Rejected => "rejected",
            Status::Superseded => "superseded",
        }
    }
}

pub struct Rfc {
    pub path: String,
    pub number: String,
    pub title: String,
    pub status: Status,
    /// The rows of its `## Rules` table: the rules it adds or changes.
    pub rules: Vec<RuleRow>,
}

/// A row of an RFC's `## Rules` table: a rule ID and the examples that test it.
pub struct RuleRow {
    pub line: usize,
    pub id: String,
    /// Paths under `litmus/`, as the row gives them.
    pub examples: Vec<String>,
}

impl Status {
    /// Whether the RFC's rules bind.
    pub fn binds(self) -> bool {
        self == Status::Accepted
    }

    /// Whether the RFC can still change, so that its rules table must match
    /// the spec and the examples.
    pub fn is_open(self) -> bool {
        matches!(self, Status::Draft | Status::Proposed)
    }
}

pub fn check(repo: &Repo, report: &mut Report) -> Vec<Rfc> {
    let mut rfcs = Vec::new();
    let mut superseded_by = Vec::new();
    let mut numbers: BTreeMap<String, String> = BTreeMap::new();
    for path in repo.paths.iter().filter(|p| p.starts_with("design/rfcs/")) {
        if path == "design/rfcs/README.md" {
            continue;
        }
        let Some(number) = parse_path(path) else {
            report.error(
                path,
                None,
                "expected design/rfcs/NNNN-slug.md, with a four-digit number and a kebab-case slug",
            );
            continue;
        };
        if let Some(other) = numbers.insert(number.to_string(), path.clone()) {
            report.error(path, None, format!("RFC {number} is already {other}"));
        }
        let text = repo.text(path).unwrap_or_default();
        let prefix = format!("RFC {number}: ");
        let title = markdown::title(text)
            .and_then(|t| t.strip_prefix(&prefix))
            .map(str::trim)
            .filter(|t| !t.is_empty());
        let Some(title) = title else {
            report.error(
                path,
                Some(1),
                format!("the RFC starts with `# {prefix}<title>`"),
            );
            continue;
        };
        let Some((line, status)) = markdown::field(text, "Status") else {
            report.error(path, None, "missing the `- **Status:** <status>` line");
            continue;
        };
        let Some(status) = Status::parse(status) else {
            report.error(
                path,
                Some(line),
                format!(
                    "`{status}` is not a status: draft, proposed, accepted, rejected or superseded"
                ),
            );
            continue;
        };
        if path == TEMPLATE && status != Status::Draft {
            report.error(path, Some(line), "the template's status stays `draft`");
        }
        if number == "0000" && path != TEMPLATE {
            report.error(path, None, "RFC 0000 is reserved for the template");
        }
        if status == Status::Superseded {
            match markdown::field(text, "Superseded by") {
                Some((line, by)) => superseded_by.push((path.clone(), line, by.to_string())),
                None => report.error(
                    path,
                    Some(line),
                    "a superseded RFC names its replacement: `- **Superseded by:** RFC NNNN`",
                ),
            }
        }
        let rules = if path == TEMPLATE {
            Vec::new()
        } else {
            rules_table(path, text, report)
        };
        rfcs.push(Rfc {
            path: path.clone(),
            number: number.to_string(),
            title: title.to_string(),
            status,
            rules,
        });
    }
    for (path, line, by) in superseded_by {
        let exists = by
            .strip_prefix("RFC ")
            .is_some_and(|n| numbers.contains_key(n) && n != "0000");
        if !exists {
            report.error(&path, Some(line), format!("`{by}` is not an existing RFC"));
        }
    }
    rfcs
}

/// The rows of the table in the `## Rules` section. The first cell of each
/// row is a rule ID in backticks; the second lists example paths in backticks.
fn rules_table(path: &str, text: &str, report: &mut Report) -> Vec<RuleRow> {
    let mut rows = Vec::new();
    let mut in_section = false;
    for line in markdown::lines(text) {
        if line.in_fence {
            continue;
        }
        if line.text.starts_with("## ") {
            in_section = line.text.trim() == "## Rules";
            continue;
        }
        let row = line.text.trim();
        if !in_section || !row.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = row.trim_matches('|').split('|').map(str::trim).collect();
        let first = cells.first().copied().unwrap_or_default();
        if first == "Rule" || first.starts_with("---") {
            continue;
        }
        let Some(id) = first
            .strip_prefix('`')
            .and_then(|c| c.strip_suffix('`'))
            .filter(|id| spec::is_valid_id(id))
        else {
            report.error(
                path,
                Some(line.number),
                "a row of the rules table starts with a rule ID in backticks",
            );
            continue;
        };
        let examples = cells
            .get(1)
            .map(|cell| backticked(cell).map(|p| p.to_string()).collect())
            .unwrap_or_default();
        rows.push(RuleRow {
            line: line.number,
            id: id.to_string(),
            examples,
        });
    }
    rows
}

/// The spans of text between pairs of backticks.
fn backticked(text: &str) -> impl Iterator<Item = &str> {
    text.split('`').skip(1).step_by(2)
}

fn parse_path(path: &str) -> Option<&str> {
    let file = path.strip_prefix("design/rfcs/")?.strip_suffix(".md")?;
    let (number, slug) = file.split_once('-')?;
    let numeric = number.len() == 4 && number.chars().all(|c| c.is_ascii_digit());
    (numeric && is_kebab(slug)).then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc(number: &str, status: &str) -> String {
        format!("# RFC {number}: Title\n\n- **Status:** {status}\n")
    }

    #[test]
    fn checks_names_titles_and_statuses() {
        let superseded =
            "# RFC 0003: Old\n\n- **Status:** superseded\n- **Superseded by:** RFC 0009\n";
        let files = [
            (TEMPLATE, rfc("0000", "draft")),
            ("design/rfcs/README.md", String::new()),
            ("design/rfcs/0001-proofs.md", rfc("0001", "accepted")),
            ("design/rfcs/0001-again.md", rfc("0001", "draft")),
            ("design/rfcs/0002-x.md", rfc("0002", "pending")),
            ("design/rfcs/0003-old.md", superseded.to_string()),
            ("design/rfcs/0004-y.md", rfc("0005", "draft")),
            ("design/rfcs/12-z.md", rfc("0012", "draft")),
        ];
        let files: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        let repo = Repo::from_files(&files);
        let mut report = Report::default();
        let rfcs = check(&repo, &mut report);
        let numbers: Vec<&str> = rfcs.iter().map(|r| r.number.as_str()).collect();
        assert!(rfcs.iter().all(|r| r.rules.is_empty()));
        assert_eq!(numbers, ["0000", "0001", "0001", "0003"]);
        assert_eq!(
            report.messages(),
            [
                "error: design/rfcs/0001-proofs.md: RFC 0001 is already design/rfcs/0001-again.md",
                "error: design/rfcs/0002-x.md:3: `pending` is not a status: draft, proposed, accepted, rejected or superseded",
                "error: design/rfcs/0004-y.md:1: the RFC starts with `# RFC 0004: <title>`",
                "error: design/rfcs/12-z.md: expected design/rfcs/NNNN-slug.md, with a four-digit number and a kebab-case slug",
                "error: design/rfcs/0003-old.md:4: `RFC 0009` is not an existing RFC",
            ]
        );
    }

    #[test]
    fn reads_the_rules_table() {
        let text = "\
# RFC 0001: T

- **Status:** proposed

## Design

| Rule | Examples |
|---|---|
| `not.this-table` | `t/a.accept.lc` |

## Rules

| Rule | Examples |
|---|---|
| `erasure.tag.kept` | `tag/kept.accept.lc`, `litmus/tag/dropped.reject.lc` |
| `erasure.lonely` | |
| Erasure | `tag/kept.accept.lc` |

## Interactions
";
        let repo = Repo::from_files(&[("design/rfcs/0001-t.md", text)]);
        let mut report = Report::default();
        let rfcs = check(&repo, &mut report);
        assert_eq!(
            report.messages(),
            [
                "error: design/rfcs/0001-t.md:17: a row of the rules table starts with a rule ID in backticks"
            ]
        );
        let rows: Vec<(usize, &str, Vec<&str>)> = rfcs[0]
            .rules
            .iter()
            .map(|r| {
                (
                    r.line,
                    r.id.as_str(),
                    r.examples.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                (
                    15,
                    "erasure.tag.kept",
                    vec!["tag/kept.accept.lc", "litmus/tag/dropped.reject.lc"]
                ),
                (16, "erasure.lonely", vec![]),
            ]
        );
    }
}
