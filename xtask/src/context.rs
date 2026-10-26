//! `cargo xtask context <rule>...`: what the repository says about a rule, in
//! one place: its text, the RFC that introduced it and that RFC's status, its
//! examples, the questions still open about it, and the trust entries its RFC
//! added. A task can start from this instead of from whole directories.

use std::path::Path;

use crate::check::{self, Checked};
use crate::litmus::{Example, Mode, Outcome};
use crate::repo::Repo;
use crate::report::Report;
use crate::spec::Rule;
use crate::trust;

pub fn run(root: &Path, queries: &[String]) -> Result<(), String> {
    let repo = Repo::load(root)?;
    let mut report = Report::default();
    let checked = check::check_repo(&repo, &mut report);
    let has_compiler = repo.exists("crates");
    let mut out = Vec::new();
    for query in queries {
        let rules = matching(&checked, query);
        if rules.is_empty() {
            return Err(format!(
                "no rule is `{query}`, and none starts with `{query}.`"
            ));
        }
        out.extend(rules.iter().map(|r| describe(&checked, has_compiler, r)));
    }
    print!("{}", out.join("\n"));
    if !report.errors.is_empty() {
        println!(
            "\nNote: `cargo xtask check` reports {} error(s), so this may be incomplete.",
            report.errors.len()
        );
    }
    Ok(())
}

/// The rule named `query`, or else every rule whose ID starts with `query.`.
fn matching<'a>(checked: &'a Checked, query: &str) -> Vec<&'a Rule> {
    let rules = &checked.spec.rules;
    let exact: Vec<&Rule> = rules.iter().filter(|r| r.id == query).collect();
    if !exact.is_empty() {
        return exact;
    }
    let prefix = format!("{}.", query.trim_end_matches('.'));
    rules.iter().filter(|r| r.id.starts_with(&prefix)).collect()
}

pub fn describe(checked: &Checked, has_compiler: bool, rule: &Rule) -> String {
    let mut out = format!("# {}\n\n", rule.id);
    let chapter = checked
        .spec
        .chapters
        .iter()
        .find(|c| c.path == rule.path)
        .map_or("", |c| c.title.as_str());
    out.push_str(&format!(
        "- Defined in: {}:{} ({chapter})\n",
        rule.path, rule.line
    ));
    let rfc = checked.introduced_by(&rule.id);
    match rfc {
        Some(rfc) => out.push_str(&format!(
            "- Introduced by: RFC {}, {} ({}), {}\n",
            rfc.number,
            rfc.title,
            rfc.status.as_str(),
            rfc.path
        )),
        None => out.push_str("- Introduced by: no RFC lists it in its `## Rules` table\n"),
    }
    out.push_str(match rfc.is_some_and(|r| r.status.binds()) {
        true => "- Binding: yes\n",
        false => "- Binding: not until its RFC is accepted\n",
    });
    out.push_str(match has_compiler {
        true => "- Implementation: not tracked yet\n",
        false => "- Implementation: none; the compiler does not exist yet\n",
    });
    out.push('\n');
    for line in rule.text.lines() {
        out.push_str(&format!("> {line}\n").replace("> \n", ">\n"));
    }

    let citing: Vec<&Example> = checked
        .examples
        .iter()
        .filter(|e| e.specs.iter().any(|(_, id)| *id == rule.id))
        .collect();
    out.push_str("\n## Examples\n\n");
    let decided: Vec<&&Example> = citing.iter().filter(|e| e.outcome.is_decided()).collect();
    if decided.is_empty() {
        out.push_str("None.\n");
    }
    for outcome in [Outcome::Accept, Outcome::Reject] {
        for e in decided.iter().filter(|e| e.outcome == outcome) {
            out.push_str(&example_line(e));
        }
    }
    out.push_str("\n## Open questions\n\n");
    let open: Vec<&&Example> = citing.iter().filter(|e| !e.outcome.is_decided()).collect();
    if open.is_empty() {
        out.push_str("No open example cites it. Check its RFC's unresolved questions and the open `decision-needed` issues.\n");
    }
    for e in open {
        out.push_str(&example_line(e));
    }

    if let Some(rfc) = rfc {
        let entries: Vec<_> = checked
            .trust
            .iter()
            .filter(|t| t.rfcs.contains(&rfc.number))
            .collect();
        out.push_str(&format!(
            "\n## Trust entries added by RFC {}\n\n",
            rfc.number
        ));
        if entries.is_empty() {
            out.push_str("None.\n");
        }
        for entry in entries {
            out.push_str(&format!(
                "- `{}` ({}:{}): {}\n",
                entry.id,
                trust::PATH,
                entry.line,
                entry.name
            ));
        }
    }
    out
}

fn example_line(e: &Example) -> String {
    let mut facts = vec![e.outcome.as_str().to_string()];
    if e.mode == Mode::Export {
        facts.push("export mode".to_string());
    }
    if let Some(issue) = e.issue {
        facts.push(format!("issue #{issue}"));
    }
    let mut line = format!(
        "- {} ({}): {}\n",
        e.path,
        facts.join(", "),
        e.question.as_deref().unwrap_or("")
    );
    for error in &e.errors {
        let expected = match &error.code {
            Some(code) if error.text.is_empty() => code.clone(),
            Some(code) => format!("{code}: {}", error.text),
            None => error.text.clone(),
        };
        line.push_str(&format!("  - error at line {}: {expected}\n", error.target));
    }
    for run in &e.runs {
        line.push_str(&format!("  - run: {} => {}\n", run.call, run.expected));
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_a_rule() {
        let rfc = "\
# RFC 0001: Erasure

- **Status:** proposed

## Rules

| Rule | Examples |
|---|---|
| `erasure.tag.kept` | `tag/kept.accept.lc` |
";
        let trust = "\
# Trust model

## Trusted components

| ID | Component | Added by |
|---|---|---|
| `trusted.erasure` | Erasure | [RFC 0001](rfcs/0001-erasure.md) |
";
        let repo = Repo::from_files(&[
            (
                "spec/README.md",
                "# Specification\n\n## Chapters\n\n- [Erasure](erasure.md)\n",
            ),
            (
                "spec/erasure.md",
                "# Erasure\n\nr[erasure.tag.kept]\nThe tag is kept\nin both programs.\n\nMore.\n",
            ),
            ("design/rfcs/0001-erasure.md", rfc),
            ("design/TRUST.md", trust),
            (
                "litmus/tag/kept.accept.lc",
                "//~ question: Is the tag kept?\n//~ spec: erasure.tag.kept\nfn f() {}\n//~ run: f() => ()\n",
            ),
            (
                "litmus/tag/lost.reject.lc",
                "//~ question: Is it lost?\n//~ spec: erasure.tag.kept\n//~ mode: export\nx //~ error: the tag is gone\n",
            ),
            (
                "litmus/tag/maybe.open.lc",
                "//~ question: Maybe?\n//~ spec: erasure.tag.kept\n//~ issue: #4\n",
            ),
        ]);
        let mut report = Report::default();
        let checked = check::check_repo(&repo, &mut report);
        assert!(report.errors.is_empty(), "{:?}", report.messages());
        assert_eq!(matching(&checked, "erasure").len(), 1);
        assert!(matching(&checked, "eras").is_empty());
        let text = describe(&checked, false, &checked.spec.rules[0]);
        assert_eq!(
            text,
            "\
# erasure.tag.kept

- Defined in: spec/erasure.md:3 (Erasure)
- Introduced by: RFC 0001, Erasure (proposed), design/rfcs/0001-erasure.md
- Binding: not until its RFC is accepted
- Implementation: none; the compiler does not exist yet

> The tag is kept
> in both programs.

## Examples

- litmus/tag/kept.accept.lc (accept): Is the tag kept?
  - run: f() => ()
- litmus/tag/lost.reject.lc (reject, export mode): Is it lost?
  - error at line 4: the tag is gone

## Open questions

- litmus/tag/maybe.open.lc (open, issue #4): Maybe?

## Trust entries added by RFC 0001

- `trusted.erasure` (design/TRUST.md:7): Erasure
"
        );
    }
}
