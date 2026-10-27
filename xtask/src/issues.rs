//! Issues: `issues/<name>.md`, one for each open question, piece of design
//! work, task, or soundness problem. Each has a title, a kind and a status,
//! and ends with a log that changes only by growing.

use crate::litmus::is_kebab;
use crate::markdown;
use crate::repo::Repo;
use crate::report::Report;
use crate::spec;

pub const README: &str = "issues/README.md";

/// What closes an issue. Declared in the order the index lists them: the most
/// urgent first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    /// Something accepted that should not be, or a hole in the trust model.
    Soundness,
    /// A question only the owner can answer.
    Decision,
    /// Design work: an RFC, spec rules, litmus examples, or a spike.
    Design,
    /// Implementation or tooling work, written as a contract.
    Task,
}

impl Kind {
    fn parse(s: &str) -> Option<Kind> {
        match s {
            "soundness" => Some(Kind::Soundness),
            "decision" => Some(Kind::Decision),
            "design" => Some(Kind::Design),
            "task" => Some(Kind::Task),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Soundness => "soundness",
            Kind::Decision => "decision",
            Kind::Design => "design",
            Kind::Task => "task",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Status {
    Open,
    /// Recorded so that it is not forgotten, and not scheduled.
    Later,
    Closed,
}

impl Status {
    fn parse(s: &str) -> Option<Status> {
        match s {
            "open" => Some(Status::Open),
            "later" => Some(Status::Later),
            "closed" => Some(Status::Closed),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Later => "later",
            Status::Closed => "closed",
        }
    }
}

pub struct Issue {
    pub path: String,
    /// The file name without `.md`, which is the issue's permanent name.
    pub name: String,
    pub title: String,
    pub kind: Kind,
    pub status: Status,
    /// What waits on it, as its `Blocks` line says.
    pub blocks: Option<String>,
    /// The rules its `Rules` line cites, with that line's number.
    pub rules: Vec<(usize, String)>,
    /// What closed it, as its `Closed by` line says, with that line's number.
    pub closed_by: Option<(usize, String)>,
    /// Whether the last entry of its log quotes someone.
    pub last_entry_quotes: bool,
}

impl Issue {
    /// The number of the RFC that its `Closed by` line names first, if any.
    pub fn closed_by_rfc(&self) -> Option<&str> {
        let (_, by) = self.closed_by.as_ref()?;
        let number = by.trim_start_matches('[').strip_prefix("RFC ")?.get(..4)?;
        number.chars().all(|c| c.is_ascii_digit()).then_some(number)
    }

    /// Whether its `Closed by` line says that the owner closed it.
    pub fn closed_by_owner(&self) -> bool {
        self.closed_by.as_ref().is_some_and(|(_, by)| {
            by.strip_prefix("the owner")
                .or_else(|| by.strip_prefix("The owner"))
                .is_some()
        })
    }
}

pub fn check(repo: &Repo, report: &mut Report) -> Vec<Issue> {
    let mut issues = Vec::new();
    for path in repo.paths.iter().filter(|p| p.starts_with("issues/")) {
        if path == README {
            continue;
        }
        let Some(name) = parse_path(path) else {
            report.error(
                path,
                None,
                "expected issues/<name>.md, with a kebab-case name",
            );
            continue;
        };
        let text = repo.text(path).unwrap_or_default();
        if let Some(issue) = parse(path, name, text, report) {
            issues.push(issue);
        }
    }
    issues.sort_by(|a, b| {
        (a.status, a.kind, &a.title, &a.name).cmp(&(b.status, b.kind, &b.title, &b.name))
    });
    issues
}

fn parse_path(path: &str) -> Option<&str> {
    let name = path.strip_prefix("issues/")?.strip_suffix(".md")?;
    is_kebab(name).then_some(name)
}

fn parse(path: &str, name: &str, text: &str, report: &mut Report) -> Option<Issue> {
    let Some(title) = markdown::title(text).filter(|t| !t.is_empty()) else {
        report.error(path, Some(1), "an issue starts with `# <title>`");
        return None;
    };
    let Some((line, kind)) = markdown::field(text, "Kind") else {
        report.error(path, None, "missing the `- **Kind:** <kind>` line");
        return None;
    };
    let Some(kind) = Kind::parse(kind) else {
        report.error(
            path,
            Some(line),
            format!("`{kind}` is not a kind: decision, design, task or soundness"),
        );
        return None;
    };
    let Some((status_line, status)) = markdown::field(text, "Status") else {
        report.error(path, None, "missing the `- **Status:** <status>` line");
        return None;
    };
    let Some(status) = Status::parse(status) else {
        report.error(
            path,
            Some(status_line),
            format!("`{status}` is not a status: open, later or closed"),
        );
        return None;
    };
    let closed_by = markdown::field(text, "Closed by")
        .filter(|(_, by)| !by.is_empty())
        .map(|(line, by)| (line, by.to_string()));
    match (status, &closed_by) {
        (Status::Closed, None) => report.error(
            path,
            Some(status_line),
            "a closed issue says what closed it: `- **Closed by:** ...`",
        ),
        (Status::Open | Status::Later, Some((line, _))) => report.error(
            path,
            Some(*line),
            "only a closed issue has a `Closed by` line",
        ),
        _ => {}
    }
    let mut rules = Vec::new();
    if let Some((line, value)) = markdown::field(text, "Rules") {
        let ids: Vec<&str> = value.split('`').skip(1).step_by(2).collect();
        if ids.is_empty() {
            report.error(path, Some(line), "list the rule IDs in backticks");
        }
        for id in ids {
            if spec::is_valid_id(id) {
                rules.push((line, id.to_string()));
            } else {
                report.error(path, Some(line), format!("`{id}` is not a valid rule ID"));
            }
        }
    }
    let blocks = markdown::field(text, "Blocks")
        .map(|(_, b)| b.to_string())
        .filter(|b| !b.is_empty());
    let last_entry_quotes = check_log(path, text, report);
    Some(Issue {
        path: path.to_string(),
        name: name.to_string(),
        title: title.to_string(),
        kind,
        status,
        blocks,
        rules,
        closed_by,
        last_entry_quotes,
    })
}

/// The `## Log` section comes last, so that a PR adds to it only at the end,
/// and holds entries that each start with `### YYYY-MM-DD, <where>`. Returns
/// whether the last entry quotes someone, in a `>` block.
fn check_log(path: &str, text: &str, report: &mut Report) -> bool {
    let lines = markdown::lines(text);
    let Some(start) = lines
        .iter()
        .position(|l| !l.in_fence && l.text.trim_end() == "## Log")
    else {
        report.error(path, None, "an issue ends with a `## Log` section");
        return false;
    };
    let mut quotes = false;
    let mut in_entry = false;
    for line in lines[start + 1..].iter().filter(|l| !l.in_fence) {
        let text = line.text.trim_end();
        if text.starts_with("## ") || text.starts_with("# ") {
            report.error(
                path,
                Some(line.number),
                "the `## Log` section comes last, so that new entries go at the end",
            );
            return false;
        }
        if let Some(heading) = text.strip_prefix("### ") {
            if !starts_with_date(heading) {
                report.error(
                    path,
                    Some(line.number),
                    "a log entry starts with `### YYYY-MM-DD, <where>`",
                );
            }
            in_entry = true;
            quotes = false;
        } else if !in_entry && !text.is_empty() {
            report.error(
                path,
                Some(line.number),
                "the log holds only entries, each starting with `### YYYY-MM-DD, <where>`",
            );
            return false;
        } else if text.trim_start().starts_with('>') {
            quotes = true;
        }
    }
    quotes
}

fn starts_with_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 10
        && bytes[..10].iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// The lines of the `## Log` section after its heading, with trailing
/// whitespace and trailing blank lines removed.
fn log(text: &str) -> Option<Vec<&str>> {
    let lines = markdown::lines(text);
    let start = lines
        .iter()
        .position(|l| !l.in_fence && l.text.trim_end() == "## Log")?;
    let mut log: Vec<&str> = lines[start + 1..]
        .iter()
        .map(|l| l.text.trim_end())
        .collect();
    while log.last().is_some_and(|l| l.is_empty()) {
        log.pop();
    }
    Some(log)
}

/// Whether `new` keeps the log of `old` unchanged, adding to it only at the end.
pub fn log_only_grows(old: &str, new: &str) -> bool {
    match (log(old), log(new)) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(old), Some(new)) => new.starts_with(&old),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUE: &str = "\
# Runtime semantics

- **Kind:** decision
- **Status:** open
- **Blocks:** the first build task
- **Rules:** `logic.boundary.calls`, `logic.boundary.writes`

## Question

What happens at run time?

## Log

### 2026-10-25, PR #10

Opened.

```text
## Not a heading
```
";

    fn checked(files: &[(&str, &str)]) -> (Vec<Issue>, Vec<String>) {
        let repo = Repo::from_files(files);
        let mut report = Report::default();
        let issues = check(&repo, &mut report);
        (issues, report.messages())
    }

    #[test]
    fn reads_an_issue() {
        let (issues, messages) = checked(&[(README, ""), ("issues/runtime.md", ISSUE)]);
        assert!(messages.is_empty(), "{messages:?}");
        let issue = &issues[0];
        assert_eq!(issue.name, "runtime");
        assert_eq!(issue.title, "Runtime semantics");
        assert_eq!((issue.kind, issue.status), (Kind::Decision, Status::Open));
        assert_eq!(issue.blocks.as_deref(), Some("the first build task"));
        assert_eq!(
            issue.rules,
            [
                (6, "logic.boundary.calls".to_string()),
                (6, "logic.boundary.writes".to_string())
            ]
        );
        assert!(!issue.last_entry_quotes);
    }

    #[test]
    fn checks_names_fields_and_logs() {
        let closed = "# C\n\n- **Kind:** task\n- **Status:** closed\n\n## Log\n";
        let unclosed =
            "# U\n\n- **Kind:** task\n- **Status:** open\n- **Closed by:** PR 3\n\n## Log\n";
        let late_section = "# L\n\n- **Kind:** design\n- **Status:** later\n\n## Log\n\n### 2026-01-02, here\n\n## After\n";
        let loose = "# P\n\n- **Kind:** design\n- **Status:** open\n\n## Log\n\nA note.\n";
        let undated = "# D\n\n- **Kind:** design\n- **Status:** open\n\n## Log\n\n### Yesterday\n";
        let (issues, messages) = checked(&[
            ("issues/Bad_Name.md", ISSUE),
            (
                "issues/kind.md",
                "# K\n\n- **Kind:** idea\n- **Status:** open\n",
            ),
            (
                "issues/no-log.md",
                "# N\n\n- **Kind:** task\n- **Status:** open\n",
            ),
            ("issues/closed.md", closed),
            ("issues/unclosed.md", unclosed),
            ("issues/late-section.md", late_section),
            ("issues/loose.md", loose),
            ("issues/undated.md", undated),
            (
                "issues/rules.md",
                "# R\n\n- **Kind:** task\n- **Status:** open\n- **Rules:** none\n\n## Log\n",
            ),
        ]);
        assert_eq!(
            messages,
            [
                "error: issues/Bad_Name.md: expected issues/<name>.md, with a kebab-case name",
                "error: issues/closed.md:4: a closed issue says what closed it: `- **Closed by:** ...`",
                "error: issues/kind.md:3: `idea` is not a kind: decision, design, task or soundness",
                "error: issues/late-section.md:10: the `## Log` section comes last, so that new entries go at the end",
                "error: issues/loose.md:8: the log holds only entries, each starting with `### YYYY-MM-DD, <where>`",
                "error: issues/no-log.md: an issue ends with a `## Log` section",
                "error: issues/rules.md:5: list the rule IDs in backticks",
                "error: issues/unclosed.md:5: only a closed issue has a `Closed by` line",
                "error: issues/undated.md:8: a log entry starts with `### YYYY-MM-DD, <where>`",
            ]
        );
        assert_eq!(issues.len(), 7);
    }

    #[test]
    fn names_what_closed_it() {
        let issue = |by: &str, log: &str| {
            let text = format!(
                "# T\n\n- **Kind:** decision\n- **Status:** closed\n- **Closed by:** {by}\n\n## Log\n{log}"
            );
            let (mut issues, _) = checked(&[("issues/t.md", text.as_str())]);
            issues.remove(0)
        };
        assert_eq!(issue("RFC 0003", "").closed_by_rfc(), Some("0003"));
        let linked = issue("[RFC 0004](../design/rfcs/0004-x.md)", "");
        assert_eq!(linked.closed_by_rfc(), Some("0004"));
        let owner = issue(
            "the owner, in review",
            "\n### 2026-10-27, PR #12\n\n> Keep it.\n",
        );
        assert!(owner.closed_by_owner() && owner.last_entry_quotes);
        assert_eq!(owner.closed_by_rfc(), None);
        let earlier = issue(
            "the owner",
            "\n### 2026-10-26, a\n\n> Quoted.\n\n### 2026-10-27, b\n\nNo quote.\n",
        );
        assert!(!earlier.last_entry_quotes);
    }

    #[test]
    fn logs_only_grow() {
        let grown = format!("{ISSUE}\n### 2026-10-27, PR #11\n\nMore.\n");
        assert!(log_only_grows(ISSUE, &grown));
        assert!(log_only_grows(
            ISSUE,
            &ISSUE.replace("What happens", "What runs")
        ));
        assert!(log_only_grows(ISSUE, &format!("{ISSUE}\n\n  \n")));
        assert!(!log_only_grows(&grown, ISSUE));
        assert!(!log_only_grows(
            ISSUE,
            &ISSUE.replace("Opened.", "Opened early.")
        ));
        assert!(!log_only_grows(
            ISSUE,
            &ISSUE.replace("## Log", "## History")
        ));
        assert!(log_only_grows("# No log yet\n", ISSUE));
    }
}
