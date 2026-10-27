//! `cargo xtask check`: every structural check over the repository, plus the
//! cross-references between them (rule coverage, citations, error codes,
//! issues, and links).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::diagnostics::{self, Diagnostic};
use crate::issues::{self, Issue, Kind, Status};
use crate::litmus::{self, Example, Outcome};
use crate::markdown;
use crate::repo::{self, Repo};
use crate::report::Report;
use crate::rfc::{self, Rfc};
use crate::spec::{self, Spec};
use crate::trust::{self, Entry};

pub struct Checked {
    pub spec: Spec,
    pub examples: Vec<Example>,
    pub rfcs: Vec<Rfc>,
    pub diagnostics: Vec<Diagnostic>,
    pub trust: Vec<Entry>,
    /// In the order the index lists them: by status, then kind, then title.
    pub issues: Vec<Issue>,
}

impl Checked {
    /// The RFC a rule comes from: an accepted RFC that lists it, or else the
    /// first other RFC that does.
    pub fn introduced_by(&self, rule: &str) -> Option<&Rfc> {
        let listing = || {
            self.rfcs.iter().filter(move |r| {
                r.path != rfc::TEMPLATE && r.rules.iter().any(|row| row.id == rule)
            })
        };
        listing()
            .find(|r| r.status.binds())
            .or_else(|| listing().find(|r| r.status.is_open()))
            .or_else(|| listing().next())
    }

    /// An RFC still being decided that lists a rule an accepted RFC
    /// introduced. The rule's text here is then that RFC's proposed revision,
    /// not the accepted text.
    pub fn revised_by(&self, rule: &str) -> Option<&Rfc> {
        if !self.introduced_by(rule)?.status.binds() {
            return None;
        }
        self.rfcs.iter().find(|r| {
            r.path != rfc::TEMPLATE
                && r.status.is_open()
                && r.rules.iter().any(|row| row.id == rule)
        })
    }

    /// Whether a rule's text, as written here, binds: an accepted RFC
    /// introduced it, and no RFC still being decided revises it.
    pub fn binds(&self, rule: &str) -> bool {
        self.introduced_by(rule).is_some_and(|r| r.status.binds())
            && self.revised_by(rule).is_none()
    }
}

pub fn run(root: &Path) -> Result<(), String> {
    let repo = Repo::load(root)?;
    let mut report = Report::default();
    let checked = check_repo(&repo, &mut report);
    report.finish()?;
    let count = |outcome| {
        checked
            .examples
            .iter()
            .filter(|e| e.outcome == outcome)
            .count()
    };
    println!(
        "ok: {} rules in {} chapters; {} litmus examples ({} accept, {} reject, {} open); {} RFCs; {} error codes",
        checked.spec.rules.len(),
        checked.spec.chapters.len(),
        checked.examples.len(),
        count(Outcome::Accept),
        count(Outcome::Reject),
        count(Outcome::Open),
        checked
            .rfcs
            .iter()
            .filter(|r| r.path != rfc::TEMPLATE)
            .count(),
        checked.diagnostics.len(),
    );
    let issues = |status| checked.issues.iter().filter(|i| i.status == status).count();
    println!(
        "issues: {} open, {} later, {} closed",
        issues(Status::Open),
        issues(Status::Later),
        issues(Status::Closed)
    );
    if !report.warnings.is_empty() {
        println!("{} warning(s)", report.warnings.len());
    }
    Ok(())
}

pub fn check_repo(repo: &Repo, report: &mut Report) -> Checked {
    let spec = spec::check(repo, report);
    let examples = litmus::check(repo, report);
    let rfcs = rfc::check(repo, report);
    let diagnostics = diagnostics::check(repo, report);
    let trust = trust::check(repo, report);
    let issues = issues::check(repo, report);
    check_coverage(&spec, &examples, report);
    check_provenance(&spec, &examples, &rfcs, report);
    check_trust_origins(&trust, &rfcs, report);
    check_issues(&issues, &spec, &examples, &rfcs, report);
    check_error_codes(&examples, &diagnostics, report);
    check_links(repo, report);
    Checked {
        spec,
        examples,
        rfcs,
        diagnostics,
        trust,
        issues,
    }
}

/// An issue cites rules that exist, and is closed by something that exists. A
/// decision closes only when the owner decides it: by an accepted RFC, or by
/// the owner's words, quoted in the last entry of its log. An example links an
/// issue that exists, and an open example links a decision that is not closed.
fn check_issues(
    issues: &[Issue],
    spec: &Spec,
    examples: &[Example],
    rfcs: &[Rfc],
    report: &mut Report,
) {
    let ids: BTreeSet<&str> = spec.rules.iter().map(|r| r.id.as_str()).collect();
    for issue in issues {
        for (line, id) in &issue.rules {
            if !ids.contains(id.as_str()) {
                report.error(
                    &issue.path,
                    Some(*line),
                    format!("`{id}` is not a rule in the spec"),
                );
            }
        }
        let Some((line, _)) = &issue.closed_by else {
            continue;
        };
        let line = Some(*line);
        match issue.closed_by_rfc() {
            Some(number) => match rfcs
                .iter()
                .find(|r| r.path != rfc::TEMPLATE && r.number == number)
            {
                None => report.error(
                    &issue.path,
                    line,
                    format!("RFC {number} does not exist"),
                ),
                Some(rfc) if issue.kind == Kind::Decision && !rfc.status.binds() => report.error(
                    &issue.path,
                    line,
                    format!(
                        "a decision closes when the owner decides it, and RFC {number} is {}, not accepted",
                        rfc.status.as_str()
                    ),
                ),
                Some(_) => {}
            },
            None if issue.kind == Kind::Decision
                && !(issue.closed_by_owner() && issue.last_entry_quotes) =>
            {
                report.error(
                    &issue.path,
                    line,
                    "a decision is closed by an accepted RFC (`RFC NNNN`), or by `the owner` with their words quoted in the last log entry",
                )
            }
            None => {}
        }
    }
    let by_name: BTreeMap<&str, &Issue> = issues.iter().map(|i| (i.name.as_str(), i)).collect();
    for example in examples {
        let Some(name) = &example.issue else {
            continue;
        };
        let open = example.outcome == Outcome::Open;
        match by_name.get(name.as_str()) {
            None => report.error(
                &example.path,
                None,
                format!("`//~ issue: {name}` names no issue: there is no issues/{name}.md"),
            ),
            Some(issue) if open && issue.kind != Kind::Decision => report.error(
                &example.path,
                None,
                format!(
                    "an open example links the decision that will decide it, and issues/{name}.md is a {} issue",
                    issue.kind.as_str()
                ),
            ),
            Some(issue) if open && issue.status == Status::Closed => report.error(
                &example.path,
                None,
                format!(
                    "issues/{name}.md is closed: decide this example, or link the decision that still will"
                ),
            ),
            Some(_) => {}
        }
    }
}

/// Every trust entry names, in its last column, at least one RFC that exists.
fn check_trust_origins(trust: &[Entry], rfcs: &[Rfc], report: &mut Report) {
    for entry in trust {
        if entry.rfcs.is_empty() {
            report.error(
                trust::PATH,
                Some(entry.line),
                format!("`{}` names no RFC that added it", entry.id),
            );
        }
        for number in &entry.rfcs {
            if !rfcs
                .iter()
                .any(|r| r.path != rfc::TEMPLATE && &r.number == number)
            {
                report.error(
                    trust::PATH,
                    Some(entry.line),
                    format!("`{}` names RFC {number}, which does not exist", entry.id),
                );
            }
        }
    }
}

/// Every rule is listed in the `## Rules` table of the RFC that introduced it.
/// The table of an RFC still being decided must match the spec and the
/// examples, and an example's `//~ rfc:` must name an RFC.
fn check_provenance(spec: &Spec, examples: &[Example], rfcs: &[Rfc], report: &mut Report) {
    let rfcs: Vec<&Rfc> = rfcs.iter().filter(|r| r.path != rfc::TEMPLATE).collect();
    let listed: BTreeSet<&str> = rfcs
        .iter()
        .flat_map(|r| r.rules.iter().map(|row| row.id.as_str()))
        .collect();
    for rule in &spec.rules {
        if !listed.contains(rule.id.as_str()) {
            report.error(
                &rule.path,
                Some(rule.line),
                format!(
                    "rule `{}` is listed in no RFC's `## Rules` table; the RFC that introduces a rule lists it there",
                    rule.id
                ),
            );
        }
    }
    let ids: BTreeSet<&str> = spec.rules.iter().map(|r| r.id.as_str()).collect();
    let by_path: BTreeMap<&str, &Example> = examples.iter().map(|e| (e.path.as_str(), e)).collect();
    for rfc in rfcs.iter().filter(|r| r.status.is_open()) {
        for row in &rfc.rules {
            if !ids.contains(row.id.as_str()) {
                report.error(
                    &rfc.path,
                    Some(row.line),
                    format!("`{}` is not a rule in the spec", row.id),
                );
            }
            for listed in &row.examples {
                let path = match listed.strip_prefix("litmus/") {
                    Some(_) => listed.clone(),
                    None => format!("litmus/{listed}"),
                };
                match by_path.get(path.as_str()) {
                    None => report.error(
                        &rfc.path,
                        Some(row.line),
                        format!("there is no litmus example `{listed}`"),
                    ),
                    Some(e) if !e.specs.iter().any(|(_, id)| *id == row.id) => report.error(
                        &rfc.path,
                        Some(row.line),
                        format!("`{listed}` does not cite `{}`", row.id),
                    ),
                    Some(_) => {}
                }
            }
        }
    }
    let numbers: BTreeSet<&str> = rfcs.iter().map(|r| r.number.as_str()).collect();
    for example in examples {
        if let Some(number) = &example.rfc
            && !numbers.contains(number.as_str())
        {
            report.error(
                &example.path,
                None,
                format!("`//~ rfc: {number}` names no RFC"),
            );
        }
    }
}

#[derive(Default)]
struct Citations {
    accept: usize,
    reject: usize,
    /// Accepted examples that check acceptance only, with no run expectations.
    accept_static: usize,
}

/// Every rule needs a decided example; citations must name real rules; and a
/// rule whose examples sit on only one side of its boundary gets a warning.
fn check_coverage(spec: &Spec, examples: &[Example], report: &mut Report) {
    let mut cited: BTreeMap<&str, Citations> = spec
        .rules
        .iter()
        .map(|r| (r.id.as_str(), Citations::default()))
        .collect();
    for example in examples {
        for (line, id) in &example.specs {
            let Some(citations) = cited.get_mut(id.as_str()) else {
                report.error(
                    &example.path,
                    Some(*line),
                    format!("no spec rule is named `{id}`"),
                );
                continue;
            };
            match example.outcome {
                Outcome::Accept => {
                    citations.accept += 1;
                    if example.runs.is_empty() {
                        citations.accept_static += 1;
                    }
                }
                Outcome::Reject => citations.reject += 1,
                Outcome::Open => {}
            }
        }
    }
    for rule in &spec.rules {
        let c = &cited[rule.id.as_str()];
        let line = Some(rule.line);
        if c.accept + c.reject == 0 {
            report.error(
                &rule.path,
                line,
                format!("rule `{}` has no decided litmus example citing it", rule.id),
            );
        } else if c.accept == 0 {
            report.warning(
                &rule.path,
                line,
                format!(
                    "rule `{}` has rejected examples but no accepted near-miss",
                    rule.id
                ),
            );
        } else if c.reject == 0 && c.accept_static > 0 {
            report.warning(
                &rule.path,
                line,
                format!(
                    "rule `{}` has accepted examples but no rejected near-miss; add one if it is a legality rule",
                    rule.id
                ),
            );
        }
    }
}

fn check_error_codes(examples: &[Example], diagnostics: &[Diagnostic], report: &mut Report) {
    let codes = diagnostics::codes(diagnostics);
    for example in examples {
        for error in &example.errors {
            if let Some(code) = &error.code
                && !codes.contains(code.as_str())
            {
                report.error(
                    &example.path,
                    Some(error.line),
                    format!("{code} has no page: add diagnostics/{code}.md"),
                );
            }
        }
    }
}

/// Relative links in Markdown files must point at files or directories that
/// exist, so that they work on GitHub and on the website.
fn check_links(repo: &Repo, report: &mut Report) {
    for (path, text) in repo.texts_matching("", ".md") {
        for (line, target) in markdown::links(text) {
            let file = target.split(['#', '?']).next().unwrap_or("");
            // `{{...}}` targets are placeholders the site builder fills in.
            if markdown::is_external(&target) || file.is_empty() || file.starts_with("{{") {
                continue;
            }
            if file.starts_with('/') {
                report.error(
                    path,
                    Some(line),
                    format!("use a relative link instead of `{target}`"),
                );
                continue;
            }
            match repo::resolve(path, file) {
                Some(resolved) if repo.exists(&resolved) => {}
                Some(_) => report.error(path, Some(line), format!("broken link `{target}`")),
                None => report.error(
                    path,
                    Some(line),
                    format!("`{target}` leaves the repository"),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC_README: &str = "# Specification\n\n## Chapters\n\n- [Erasure](erasure.md)\n";
    const CHAPTER: &str = "\
# Erasure

r[erasure.tag.kept]
The tag is kept.

r[erasure.order.left-to-right]
Operands run left to right.

r[erasure.payload.gone]
Payloads disappear.

r[erasure.lonely]
Nothing cites this.
";

    const TRUST: (&str, &str) = ("design/TRUST.md", "# Trust model\n");
    /// The decision that the open examples of these tests link.
    const ISSUE: (&str, &str) = (
        "issues/lonely.md",
        "# Lonely\n\n- **Kind:** decision\n- **Status:** open\n\n## Log\n",
    );
    /// An accepted RFC listing every rule of CHAPTER, so that coverage tests
    /// see no provenance errors.
    const RFC: (&str, &str) = (
        "design/rfcs/0001-erasure.md",
        "# RFC 0001: Erasure\n\n- **Status:** accepted\n\n## Rules\n\n| Rule | Examples |\n|---|---|\n\
         | `erasure.tag.kept` | |\n| `erasure.order.left-to-right` | |\n| `erasure.payload.gone` | |\n| `erasure.lonely` | |\n",
    );

    fn example(outcome: &str, spec: &str, body: &str) -> String {
        let issue = if outcome == "open" {
            "//~ issue: lonely\n"
        } else {
            ""
        };
        format!("//~ question: q\n//~ spec: {spec}\n{issue}{body}")
    }

    #[test]
    fn coverage_citations_and_codes() {
        let accept_tag = example("accept", "erasure.tag.kept", "fn f() {}\n");
        let reject_tag = example(
            "reject",
            "erasure.tag.kept",
            "fn f() {} //~ error[L0001]: e\n",
        );
        let run_order = example(
            "accept",
            "erasure.order.left-to-right",
            "fn f() {}\n//~ run: f() => ()\n",
        );
        let reject_payload = example("reject", "erasure.payload.gone", "fn f() {} //~ error: e\n");
        let open_lonely = example("open", "erasure.lonely, erasure.missing", "fn f() {}\n");
        let repo = Repo::from_files(&[
            ("spec/README.md", SPEC_README),
            ("spec/erasure.md", CHAPTER),
            ("litmus/tag/kept.accept.lc", &accept_tag),
            ("litmus/tag/dropped.reject.lc", &reject_tag),
            ("litmus/order/calls.accept.lc", &run_order),
            ("litmus/payload/used.reject.lc", &reject_payload),
            ("litmus/lonely/q.open.lc", &open_lonely),
            TRUST,
            RFC,
            ISSUE,
        ]);
        let mut report = Report::default();
        check_repo(&repo, &mut report);
        assert_eq!(
            report.messages(),
            [
                "error: litmus/lonely/q.open.lc:2: no spec rule is named `erasure.missing`",
                "error: spec/erasure.md:12: rule `erasure.lonely` has no decided litmus example citing it",
                "error: litmus/tag/dropped.reject.lc:3: L0001 has no page: add diagnostics/L0001.md",
                "warning: spec/erasure.md:9: rule `erasure.payload.gone` has rejected examples but no accepted near-miss",
            ]
        );
    }

    #[test]
    fn static_accepts_without_rejects_warn() {
        let accept = example("accept", "erasure.tag.kept", "fn f() {}\n");
        let chapter = "# Erasure\n\nr[erasure.tag.kept]\nThe tag is kept.\n";
        let repo = Repo::from_files(&[
            ("spec/README.md", SPEC_README),
            ("spec/erasure.md", chapter),
            ("litmus/tag/kept.accept.lc", &accept),
            TRUST,
            RFC,
        ]);
        let mut report = Report::default();
        check_repo(&repo, &mut report);
        assert_eq!(
            report.messages(),
            [
                "warning: spec/erasure.md:3: rule `erasure.tag.kept` has accepted examples but no rejected near-miss; add one if it is a legality rule"
            ]
        );
    }

    #[test]
    fn rules_come_from_rfcs() {
        let chapter =
            "# Erasure\n\nr[erasure.tag.kept]\nThe tag is kept.\n\nr[erasure.lonely]\nUnlisted.\n";
        let proposed = "\
# RFC 0002: More

- **Status:** proposed

## Rules

| Rule | Examples |
|---|---|
| `erasure.tag.kept` | `tag/kept.accept.lc`, `litmus/tag/other.accept.lc`, `tag/missing.accept.lc` |
| `erasure.gone` | |
";
        // Accepted RFCs are not edited, so their tables are not re-checked.
        let accepted = "# RFC 0001: Old\n\n- **Status:** accepted\n\n## Rules\n\n| Rule | Examples |\n|---|---|\n| `erasure.removed` | `x.accept.lc` |\n";
        let kept = "//~ question: q\n//~ spec: erasure.tag.kept\n//~ rfc: 0002\n";
        let other = "//~ question: q\n//~ spec: erasure.lonely\n//~ rfc: 9999\n";
        let repo = Repo::from_files(&[
            ("spec/README.md", SPEC_README),
            ("spec/erasure.md", chapter),
            ("design/rfcs/0001-old.md", accepted),
            ("design/rfcs/0002-more.md", proposed),
            ("litmus/tag/kept.accept.lc", kept),
            ("litmus/tag/other.accept.lc", other),
            TRUST,
        ]);
        let mut report = Report::default();
        let checked = check_repo(&repo, &mut report);
        let errors: Vec<String> = report
            .messages()
            .into_iter()
            .filter(|m| m.starts_with("error"))
            .collect();
        assert_eq!(
            errors,
            [
                "error: spec/erasure.md:6: rule `erasure.lonely` is listed in no RFC's `## Rules` table; the RFC that introduces a rule lists it there",
                "error: design/rfcs/0002-more.md:9: `litmus/tag/other.accept.lc` does not cite `erasure.tag.kept`",
                "error: design/rfcs/0002-more.md:9: there is no litmus example `tag/missing.accept.lc`",
                "error: design/rfcs/0002-more.md:10: `erasure.gone` is not a rule in the spec",
                "error: litmus/tag/other.accept.lc: `//~ rfc: 9999` names no RFC",
            ]
        );
        assert_eq!(
            checked
                .introduced_by("erasure.tag.kept")
                .map(|r| r.number.as_str()),
            Some("0002")
        );
        assert!(checked.introduced_by("erasure.lonely").is_none());
    }

    #[test]
    fn proposed_revisions_do_not_bind() {
        let chapter = "# Erasure\n\nr[erasure.tag.kept]\nRevised text.\n\nr[erasure.order.left-to-right]\nUnchanged.\n";
        let accepted = "# RFC 0001: Old\n\n- **Status:** accepted\n\n## Rules\n\n| Rule | Examples |\n|---|---|\n\
                        | `erasure.tag.kept` | |\n| `erasure.order.left-to-right` | |\n";
        let revising = "# RFC 0002: Revision\n\n- **Status:** proposed\n\n## Rules\n\n| Rule | Examples |\n|---|---|\n\
                        | `erasure.tag.kept` | |\n";
        let repo = Repo::from_files(&[
            ("spec/README.md", SPEC_README),
            ("spec/erasure.md", chapter),
            ("design/rfcs/0001-old.md", accepted),
            ("design/rfcs/0002-revision.md", revising),
            TRUST,
        ]);
        let mut report = Report::default();
        let checked = check_repo(&repo, &mut report);
        let number = |r: Option<&Rfc>| r.map(|r| r.number.clone());
        assert_eq!(
            number(checked.introduced_by("erasure.tag.kept")),
            Some("0001".into())
        );
        assert_eq!(
            number(checked.revised_by("erasure.tag.kept")),
            Some("0002".into())
        );
        assert!(!checked.binds("erasure.tag.kept"));
        assert_eq!(
            number(checked.revised_by("erasure.order.left-to-right")),
            None
        );
        assert!(checked.binds("erasure.order.left-to-right"));
    }

    #[test]
    fn issues_cite_rules_and_close_as_allowed() {
        let issue = |kind: &str, status: &str, line: &str, log: &str| {
            format!("# T\n\n- **Kind:** {kind}\n- **Status:** {status}\n{line}\n\n## Log\n{log}")
        };
        let closed =
            |kind, by: &str, log| issue(kind, "closed", &format!("- **Closed by:** {by}"), log);
        let quoted = "\n### 2026-10-27, PR #12\n\n> Keep it.\n";
        let files = [
            (
                "issues/bad-rule.md",
                issue("task", "open", "- **Rules:** `erasure.nope`", ""),
            ),
            ("issues/by-accepted.md", closed("decision", "RFC 0001", "")),
            (
                "issues/by-owner.md",
                closed("decision", "the owner, in review", quoted),
            ),
            (
                "issues/by-owner-unquoted.md",
                closed("decision", "the owner", ""),
            ),
            ("issues/by-pr.md", closed("decision", "PR #3", quoted)),
            ("issues/by-proposed.md", closed("decision", "RFC 0002", "")),
            ("issues/by-missing.md", closed("task", "RFC 0009", "")),
            ("issues/task-done.md", closed("task", "PR #3", "")),
            (
                "litmus/q/a-missing.accept.lc",
                "//~ question: q\n//~ spec: erasure.tag.kept\n//~ issue: missing-issue\n".into(),
            ),
            (
                "litmus/q/b-closed.open.lc",
                "//~ question: q\n//~ issue: by-accepted\n".into(),
            ),
            (
                "litmus/q/c-task.open.lc",
                "//~ question: q\n//~ issue: task-done\n".into(),
            ),
            (
                "litmus/q/d-fine.open.lc",
                "//~ question: q\n//~ issue: lonely\n".into(),
            ),
            (
                "design/rfcs/0002-next.md",
                "# RFC 0002: Next\n\n- **Status:** proposed\n".into(),
            ),
        ];
        let mut files: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        files.extend([
            ("spec/README.md", SPEC_README),
            ("spec/erasure.md", CHAPTER),
            TRUST,
            RFC,
            ISSUE,
        ]);
        let repo = Repo::from_files(&files);
        let mut report = Report::default();
        check_repo(&repo, &mut report);
        let messages: Vec<String> = report
            .messages()
            .into_iter()
            .filter(|m| m.contains("issue"))
            .collect();
        assert_eq!(
            messages,
            [
                "error: issues/bad-rule.md:5: `erasure.nope` is not a rule in the spec",
                "error: issues/by-owner-unquoted.md:5: a decision is closed by an accepted RFC (`RFC NNNN`), or by `the owner` with their words quoted in the last log entry",
                "error: issues/by-pr.md:5: a decision is closed by an accepted RFC (`RFC NNNN`), or by `the owner` with their words quoted in the last log entry",
                "error: issues/by-proposed.md:5: a decision closes when the owner decides it, and RFC 0002 is proposed, not accepted",
                "error: issues/by-missing.md:5: RFC 0009 does not exist",
                "error: litmus/q/a-missing.accept.lc: `//~ issue: missing-issue` names no issue: there is no issues/missing-issue.md",
                "error: litmus/q/b-closed.open.lc: issues/by-accepted.md is closed: decide this example, or link the decision that still will",
                "error: litmus/q/c-task.open.lc: an open example links the decision that will decide it, and issues/task-done.md is a task issue",
            ]
        );
    }

    #[test]
    fn trust_entries_name_existing_rfcs() {
        let trust = "# Trust model\n\n## Trusted components\n\n| ID | Component | Added by |\n|---|---|---|\n\
                     | `trusted.ok` | Fine | [RFC 0001](rfcs/0001-erasure.md) |\n\
                     | `trusted.none` | No origin | someone |\n\
                     | `trusted.ghost` | Missing origin | RFC 0009 |\n";
        let repo = Repo::from_files(&[("design/TRUST.md", trust), RFC]);
        let mut report = Report::default();
        check_repo(&repo, &mut report);
        let errors: Vec<String> = report
            .messages()
            .into_iter()
            .filter(|m| m.contains("design/TRUST.md"))
            .collect();
        assert_eq!(
            errors,
            [
                "error: design/TRUST.md:8: `trusted.none` names no RFC that added it",
                "error: design/TRUST.md:9: `trusted.ghost` names RFC 0009, which does not exist",
            ]
        );
    }

    #[test]
    fn links_must_resolve() {
        let text = "[ok](spec/README.md) [dir](spec) [anchor](#x) [web](https://x.dev) \
                    [broken](nope.md) [abs](/spec/README.md) [out](../x.md) [frag](spec/README.md#chapters)";
        let repo = Repo::from_files(&[
            ("spec/README.md", "# Specification\n\n## Chapters\n"),
            ("README.md", text),
            TRUST,
        ]);
        let mut report = Report::default();
        check_repo(&repo, &mut report);
        assert_eq!(
            report.messages(),
            [
                "error: README.md:1: broken link `nope.md`",
                "error: README.md:1: use a relative link instead of `/spec/README.md`",
                "error: README.md:1: `../x.md` leaves the repository",
            ]
        );
    }
}
