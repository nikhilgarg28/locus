//! RFCs: `design/rfcs/NNNN-slug.md`, each with a title and a status line.

use std::collections::BTreeMap;

use crate::litmus::is_kebab;
use crate::markdown;
use crate::repo::Repo;
use crate::report::Report;

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
        let Some((line, status)) = field(text, "Status") else {
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
            match field(text, "Superseded by") {
                Some((line, by)) => superseded_by.push((path.clone(), line, by.to_string())),
                None => report.error(
                    path,
                    Some(line),
                    "a superseded RFC names its replacement: `- **Superseded by:** RFC NNNN`",
                ),
            }
        }
        rfcs.push(Rfc {
            path: path.clone(),
            number: number.to_string(),
            title: title.to_string(),
            status,
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

fn parse_path(path: &str) -> Option<&str> {
    let file = path.strip_prefix("design/rfcs/")?.strip_suffix(".md")?;
    let (number, slug) = file.split_once('-')?;
    let numeric = number.len() == 4 && number.chars().all(|c| c.is_ascii_digit());
    (numeric && is_kebab(slug)).then_some(number)
}

/// The value of a `- **Name:** value` line, with its line number.
fn field<'a>(text: &'a str, name: &str) -> Option<(usize, &'a str)> {
    let prefix = format!("- **{name}:**");
    markdown::lines(text)
        .into_iter()
        .filter(|l| !l.in_fence)
        .find_map(|l| Some((l.number, l.text.strip_prefix(&prefix)?.trim())))
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
}
