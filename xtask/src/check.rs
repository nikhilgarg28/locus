//! `cargo xtask check`: every structural check over the repository, plus the
//! cross-references between them (rule coverage, citations, error codes, and
//! links).

use std::collections::BTreeMap;
use std::path::Path;

use crate::diagnostics::{self, Diagnostic};
use crate::litmus::{self, Example, Outcome};
use crate::markdown;
use crate::repo::{self, Repo};
use crate::report::Report;
use crate::rfc::{self, Rfc};
use crate::spec::{self, Spec};

pub struct Checked {
    pub spec: Spec,
    pub examples: Vec<Example>,
    pub rfcs: Vec<Rfc>,
    pub diagnostics: Vec<Diagnostic>,
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
    check_coverage(&spec, &examples, report);
    check_error_codes(&examples, &diagnostics, report);
    check_links(repo, report);
    Checked {
        spec,
        examples,
        rfcs,
        diagnostics,
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

    fn example(outcome: &str, spec: &str, body: &str) -> String {
        let issue = if outcome == "open" {
            "//~ issue: #1\n"
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
    fn links_must_resolve() {
        let text = "[ok](spec/README.md) [dir](spec) [anchor](#x) [web](https://x.dev) \
                    [broken](nope.md) [abs](/spec/README.md) [out](../x.md) [frag](spec/README.md#chapters)";
        let repo = Repo::from_files(&[
            ("spec/README.md", "# Specification\n\n## Chapters\n"),
            ("README.md", text),
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
