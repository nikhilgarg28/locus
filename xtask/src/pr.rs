//! `cargo xtask pr-check`: guards on what a branch changes relative to its
//! base. Design changes (vision, spec, RFCs) and build changes (compiler
//! crates) go in separate PRs, and a decided expectation can change only when
//! the PR declares it.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use crate::litmus::{self, Outcome};
use crate::report::Report;
use crate::spec;

pub struct Args {
    pub base: String,
    pub allow_expectation_changes: bool,
}

#[derive(Debug, PartialEq)]
pub enum Change {
    Added(String),
    Modified(String),
    Deleted(String),
    Renamed { from: String, to: String },
}

impl Change {
    fn paths(&self) -> Vec<&str> {
        match self {
            Change::Added(p) | Change::Modified(p) | Change::Deleted(p) => vec![p],
            Change::Renamed { from, to } => vec![from, to],
        }
    }
}

pub fn run(root: &Path, args: &Args) -> Result<(), String> {
    let merge_base = git(root, &["merge-base", &args.base, "HEAD"])?
        .trim()
        .to_string();
    let diff = git(root, &["diff", "--name-status", "-M", &merge_base, "HEAD"])?;
    let changes = parse_name_status(&diff)?;
    let at = |rev: &str, path: &str| git(root, &["show", &format!("{rev}:{path}")]).ok();
    let base = |path: &str| at(&merge_base, path);
    let head = |path: &str| at("HEAD", path);
    let base_spec = spec_texts(root, &merge_base, &base)?;
    let head_spec = spec_texts(root, "HEAD", &head)?;

    println!(
        "comparing HEAD with {} (merge base {})",
        args.base,
        &merge_base[..12.min(merge_base.len())]
    );
    if let Some(problem) = mixed_modes(&changes) {
        return Err(format!("error: {problem}"));
    }
    let mut found = expectation_changes(&changes, &base, &head);
    found.extend(removed_rules(&base_spec, &head_spec));
    if found.is_empty() {
        println!("ok: no decided expectation changed");
        return Ok(());
    }
    let list: String = found.iter().map(|f| format!("\n  - {f}")).collect();
    if args.allow_expectation_changes {
        println!("declared expectation changes:{list}");
        Ok(())
    } else {
        Err(format!(
            "error: this branch changes decided expectations:{list}\n\
             If the owner asked for this, label the PR `expectation-change` and justify each change in its description."
        ))
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("git output is not UTF-8: {e}"))
}

fn spec_texts(
    root: &Path,
    rev: &str,
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<(String, String)>, String> {
    let listing = git(root, &["ls-tree", "-r", "--name-only", rev, "--", "spec/"])?;
    Ok(listing
        .lines()
        .filter(|p| p.ends_with(".md"))
        .filter_map(|p| Some((p.to_string(), read(p)?)))
        .collect())
}

pub fn parse_name_status(diff: &str) -> Result<Vec<Change>, String> {
    diff.lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            let status = fields[0].chars().next().unwrap_or(' ');
            match (status, &fields[1..]) {
                ('A', [p]) => Ok(Change::Added(p.to_string())),
                ('M' | 'T', [p]) => Ok(Change::Modified(p.to_string())),
                ('D', [p]) => Ok(Change::Deleted(p.to_string())),
                ('R', [from, to]) => Ok(Change::Renamed {
                    from: from.to_string(),
                    to: to.to_string(),
                }),
                ('C', [_, to]) => Ok(Change::Added(to.to_string())),
                _ => Err(format!("unexpected line from git diff: {line}")),
            }
        })
        .collect()
}

/// A PR is either design work or build work, never both.
pub fn mixed_modes(changes: &[Change]) -> Option<String> {
    let paths: Vec<&str> = changes.iter().flat_map(Change::paths).collect();
    let design = paths
        .iter()
        .find(|p| **p == "VISION.md" || p.starts_with("spec/") || p.starts_with("design/"));
    let build = paths.iter().find(|p| p.starts_with("crates/"));
    match (design, build) {
        (Some(d), Some(b)) => Some(format!(
            "this branch changes both the design ({d}) and the compiler ({b}); split it into a design PR and a build PR"
        )),
        _ => None,
    }
}

fn decided(path: &str) -> Option<Outcome> {
    litmus::parse_path(path)
        .ok()
        .map(|(_, _, outcome)| outcome)
        .filter(|o| o.is_decided())
}

/// Removed or changed outcomes, error codes and run results of decided
/// litmus examples. Adding expectations is not a change that needs declaring.
pub fn expectation_changes(
    changes: &[Change],
    base: &dyn Fn(&str) -> Option<String>,
    head: &dyn Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut found = Vec::new();
    for change in changes {
        match change {
            Change::Added(_) => {}
            Change::Deleted(path) => {
                if decided(path).is_some() {
                    found.push(format!("deletes the decided example {path}"));
                }
            }
            Change::Modified(path) => {
                if decided(path).is_some() {
                    compare(path, path, base, head, &mut found);
                }
            }
            Change::Renamed { from, to } => {
                let Some(old) = decided(from) else { continue };
                let new = litmus::parse_path(to).ok().map(|(_, _, o)| o);
                if new == Some(old) {
                    compare(from, to, base, head, &mut found);
                } else {
                    let new = new.map_or("a non-litmus path", Outcome::as_str);
                    found.push(format!("moves {from} from {} to {new}: {to}", old.as_str()));
                }
            }
        }
    }
    found
}

fn compare(
    from: &str,
    to: &str,
    base: &dyn Fn(&str) -> Option<String>,
    head: &dyn Fn(&str) -> Option<String>,
    found: &mut Vec<String>,
) {
    let parse = |path: &str, text: Option<String>| {
        let (topic, name, outcome) = litmus::parse_path(path).ok()?;
        let mut ignored = Report::default();
        Some(litmus::parse(
            path,
            &topic,
            &name,
            outcome,
            &text?,
            &mut ignored,
        ))
    };
    let (Some(old), Some(new)) = (parse(from, base(from)), parse(to, head(to))) else {
        return;
    };
    if new.errors.len() < old.errors.len() {
        found.push(format!(
            "{to}: expects fewer errors ({} → {})",
            old.errors.len(),
            new.errors.len()
        ));
    }
    let mut new_codes: Vec<&str> = new
        .errors
        .iter()
        .filter_map(|e| e.code.as_deref())
        .collect();
    for code in old.errors.iter().filter_map(|e| e.code.as_deref()) {
        match new_codes.iter().position(|c| *c == code) {
            Some(i) => {
                new_codes.remove(i);
            }
            None => found.push(format!("{to}: no longer expects {code}")),
        }
    }
    let new_runs: BTreeSet<(&str, &str)> = new
        .runs
        .iter()
        .map(|r| (r.call.as_str(), r.expected.as_str()))
        .collect();
    for run in &old.runs {
        if !new_runs.contains(&(run.call.as_str(), run.expected.as_str())) {
            found.push(format!(
                "{to}: removes or changes `run: {} => {}`",
                run.call, run.expected
            ));
        }
    }
}

/// Rule IDs present in the base's spec and missing from the head's.
pub fn removed_rules(base: &[(String, String)], head: &[(String, String)]) -> Vec<String> {
    let ids = |files: &[(String, String)]| -> BTreeSet<String> {
        let mut ignored = Report::default();
        files
            .iter()
            .flat_map(|(path, text)| spec::rules(path, text, &mut ignored))
            .map(|r| r.id)
            .collect()
    };
    let head_ids = ids(head);
    ids(base)
        .into_iter()
        .filter(|id| !head_ids.contains(id))
        .map(|id| format!("removes the spec rule `{id}`"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn parses_git_name_status() {
        let diff = "A\tlitmus/t/a.open.lc\nM\tspec/x.md\nD\told.md\nR087\tlitmus/t/b.open.lc\tlitmus/t/b.accept.lc\n";
        assert_eq!(
            parse_name_status(diff).unwrap(),
            [
                Change::Added("litmus/t/a.open.lc".into()),
                Change::Modified("spec/x.md".into()),
                Change::Deleted("old.md".into()),
                Change::Renamed {
                    from: "litmus/t/b.open.lc".into(),
                    to: "litmus/t/b.accept.lc".into()
                },
            ]
        );
        assert!(parse_name_status("X\tweird\n").is_err());
    }

    #[test]
    fn design_and_build_stay_apart() {
        let design = Change::Modified("spec/erasure.md".into());
        let build = Change::Added("crates/kernel/src/lib.rs".into());
        let neutral = Change::Modified("litmus/t/a.accept.lc".into());
        assert!(mixed_modes(&[design, neutral]).is_none());
        let design = Change::Renamed {
            from: "design/rfcs/0001-a.md".into(),
            to: "design/rfcs/0001-b.md".into(),
        };
        let message = mixed_modes(&[design, build]).unwrap();
        assert!(message.contains("design/rfcs/0001-a.md") && message.contains("crates/kernel"));
    }

    #[test]
    fn finds_weakened_expectations() {
        let header = "//~ question: q\n//~ spec: a.b\n";
        let base: BTreeMap<&str, String> = [
            (
                "litmus/t/gone.reject.lc",
                format!("{header}x //~ error: e\n"),
            ),
            ("litmus/t/flip.accept.lc", format!("{header}x\n")),
            (
                "litmus/t/codes.reject.lc",
                format!("{header}x //~ error[L0001]: e\ny //~ error[L0002]: e\n"),
            ),
            (
                "litmus/t/runs.accept.lc",
                format!("{header}x\n//~ run: f(1) => 2\n//~ run: f(2) => 3\n"),
            ),
            (
                "litmus/t/refined.reject.lc",
                format!("{header}x //~ error: e\n"),
            ),
            (
                "litmus/t/q.open.lc",
                "//~ question: q\n//~ issue: #1\n".to_string(),
            ),
        ]
        .into_iter()
        .collect();
        let head: BTreeMap<&str, String> = [
            (
                "litmus/t/flip.reject.lc",
                format!("{header}x //~ error: e\n"),
            ),
            (
                "litmus/t/codes.reject.lc",
                format!("{header}x //~ error[L0001]: e\ny //~ error[L0003]: e\n"),
            ),
            (
                "litmus/t/runs.accept.lc",
                format!("{header}x\n//~ run: f(1) => 2\n//~ run: f(2) => 4\n//~ run: f(3) => 4\n"),
            ),
            (
                "litmus/t/refined.reject.lc",
                format!("{header}x //~ error[L0001]: e\n"),
            ),
            ("litmus/t/q.reject.lc", format!("{header}x //~ error: e\n")),
        ]
        .into_iter()
        .collect();
        let changes = [
            Change::Deleted("litmus/t/gone.reject.lc".into()),
            Change::Renamed {
                from: "litmus/t/flip.accept.lc".into(),
                to: "litmus/t/flip.reject.lc".into(),
            },
            Change::Modified("litmus/t/codes.reject.lc".into()),
            Change::Modified("litmus/t/runs.accept.lc".into()),
            Change::Modified("litmus/t/refined.reject.lc".into()),
            Change::Renamed {
                from: "litmus/t/q.open.lc".into(),
                to: "litmus/t/q.reject.lc".into(),
            },
        ];
        let base = |p: &str| base.get(p).cloned();
        let head = |p: &str| head.get(p).cloned();
        assert_eq!(
            expectation_changes(&changes, &base, &head),
            [
                "deletes the decided example litmus/t/gone.reject.lc",
                "moves litmus/t/flip.accept.lc from accept to reject: litmus/t/flip.reject.lc",
                "litmus/t/codes.reject.lc: no longer expects L0002",
                "litmus/t/runs.accept.lc: removes or changes `run: f(2) => 3`",
            ]
        );
    }

    #[test]
    fn finds_removed_rules() {
        let base = [(
            "spec/a.md".to_string(),
            "r[a.one]\nx\n\nr[a.two]\ny\n".to_string(),
        )];
        let head = [("spec/b.md".to_string(), "r[a.two]\ny\n".to_string())];
        assert_eq!(
            removed_rules(&base, &head),
            ["removes the spec rule `a.one`"]
        );
    }
}
