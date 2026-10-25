//! The specification: chapters listed in `spec/README.md`, and rules marked
//! with `r[id]` on their own line before the paragraph they govern.

use std::collections::BTreeMap;

use crate::diagnostics;
use crate::litmus::is_kebab;
use crate::markdown;
use crate::repo::{self, Repo};
use crate::report::Report;

pub const README: &str = "spec/README.md";

pub struct Chapter {
    pub path: String,
    pub title: String,
}

pub struct Rule {
    pub id: String,
    pub path: String,
    pub line: usize,
}

pub struct Spec {
    pub chapters: Vec<Chapter>,
    pub rules: Vec<Rule>,
}

/// Dotted kebab-case names with at least two segments, such as
/// `erasure.option.discriminant-kept`.
pub fn is_valid_id(id: &str) -> bool {
    let segments: Vec<&str> = id.split('.').collect();
    segments.len() >= 2 && segments.iter().all(|s| is_kebab(s))
}

/// The rule ID in a marker line such as `r[erasure.option.discriminant-kept]`,
/// or `None` if the line is not a marker. The ID is not validated.
pub fn marker(line: &str) -> Option<&str> {
    line.strip_prefix("r[")?.strip_suffix(']')
}

pub fn check(repo: &Repo, report: &mut Report) -> Spec {
    let mut spec = Spec {
        chapters: Vec::new(),
        rules: Vec::new(),
    };
    let Some(readme) = repo.text(README) else {
        report.error(README, None, "the specification's README is missing");
        return spec;
    };

    let listed = chapter_links(readme, report);
    let mut seen = BTreeMap::new();
    for (line, path) in listed {
        if seen.insert(path.clone(), line).is_some() {
            report.error(README, Some(line), format!("{path} is listed twice"));
            continue;
        }
        let Some(text) = repo.text(&path) else {
            report.error(README, Some(line), format!("{path} does not exist"));
            continue;
        };
        let title = markdown::title(text).unwrap_or_default().to_string();
        if title.is_empty() {
            report.error(&path, None, "a chapter starts with a `# ` title");
        }
        spec.chapters.push(Chapter { path, title });
    }
    for (path, _) in repo.texts_matching("spec/", ".md") {
        let flat = path.matches('/').count() == 1;
        if path != README && !(flat && seen.contains_key(path)) {
            report.error(
                path,
                None,
                "every spec chapter is a file directly in spec/, listed under `## Chapters` in spec/README.md",
            );
        }
    }
    for path in repo.paths.iter().filter(|p| p.starts_with("spec/")) {
        if !path.ends_with(".md") {
            report.error(path, None, "spec/ contains only Markdown chapters");
        }
    }

    let mut ids: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (path, text) in repo.texts_matching("spec/", ".md") {
        check_fences(path, text, false, report);
        for rule in rules(path, text, report) {
            if path == README {
                report.error(
                    path,
                    Some(rule.line),
                    "rules belong in chapters, not the README",
                );
            } else if let Some((other, line)) = ids.get(&rule.id) {
                report.error(
                    path,
                    Some(rule.line),
                    format!("rule `{}` is already defined at {other}:{line}", rule.id),
                );
            } else {
                ids.insert(rule.id.clone(), (rule.path.clone(), rule.line));
                spec.rules.push(rule);
            }
        }
    }
    spec
}

/// Links in the `## Chapters` section of the README, resolved to repository
/// paths, in order.
fn chapter_links(readme: &str, report: &mut Report) -> Vec<(usize, String)> {
    let mut in_section = false;
    let mut section = String::new();
    let mut first_line = 0;
    for line in markdown::lines(readme) {
        if !line.in_fence && line.text.starts_with("## ") {
            in_section = line.text.trim() == "## Chapters";
            if in_section {
                first_line = line.number;
            }
        } else if in_section {
            section.push_str(line.text);
        }
        // One line per README line, so that link line numbers match.
        section.push('\n');
    }
    if first_line == 0 {
        report.error(README, None, "missing the `## Chapters` section");
    }
    markdown::links(&section)
        .into_iter()
        .filter_map(|(line, target)| match repo::resolve(README, &target) {
            Some(path) => Some((line, path)),
            None => {
                report.error(
                    README,
                    Some(line),
                    format!("{target} is outside the repository"),
                );
                None
            }
        })
        .collect()
}

/// The rules marked in one file. Markers inside code blocks are ignored.
pub fn rules(path: &str, text: &str, report: &mut Report) -> Vec<Rule> {
    let lines = markdown::lines(text);
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.in_fence {
            continue;
        }
        let Some(id) = marker(line.text.trim_end()) else {
            if line.text.trim_start().starts_with("r[") {
                report.error(
                    path,
                    Some(line.number),
                    "a rule marker is `r[id]` alone at the start of a line",
                );
            }
            continue;
        };
        if !is_valid_id(id) {
            report.error(
                path,
                Some(line.number),
                format!("`{id}` is not a valid rule ID: use dotted kebab-case names, at least two"),
            );
            continue;
        }
        let body = lines[index + 1..]
            .iter()
            .find(|l| !l.text.trim().is_empty());
        let has_body = body.is_some_and(|l| {
            let text = l.text.trim_start();
            l.in_fence || !(text.starts_with('#') || marker(text.trim_end()).is_some())
        });
        if !has_body {
            report.error(
                path,
                Some(line.number),
                format!("rule `{id}` must be followed by the text it marks"),
            );
        }
        out.push(Rule {
            id: id.to_string(),
            path: path.to_string(),
            line: line.number,
        });
    }
    out
}

/// Checks that each `rust` fence declares how it is checked: `check`,
/// `reject` with an optional error code, or `prose` with a reason. With
/// `require_code`, a `reject` fence must name its code.
pub fn check_fences(path: &str, text: &str, require_code: bool, report: &mut Report) {
    const MODES: &str = "`rust check`, `rust reject Lnnnn` or `rust prose <reason>`";
    for fence in markdown::fences(text) {
        let words: Vec<&str> = fence.info.split_whitespace().collect();
        let Some(first) = words.first() else { continue };
        if !first.starts_with("rust") {
            continue;
        }
        let valid = match &words[..] {
            ["rust", "check"] => true,
            ["rust", "reject"] => !require_code,
            ["rust", "reject", code] => diagnostics::is_code(code),
            ["rust", "prose", _reason, ..] => true,
            _ => false,
        };
        if !valid {
            report.error(
                path,
                Some(fence.line),
                format!("this fence needs one of {MODES}, not `{}`", fence.info),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const README_TEXT: &str =
        "# Specification\n\n## Chapters\n\n1. [Erasure](erasure.md)\n\n## Other\n\n[x](x.md)\n";

    #[test]
    fn rule_ids() {
        assert!(is_valid_id("erasure.option.discriminant-kept"));
        assert!(is_valid_id("a.b"));
        for bad in ["a", "a..b", "A.b", "a.b_c", "a.b.", ".a"] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }

    #[test]
    fn collects_chapters_and_rules() {
        let chapter = "# Erasure\n\nr[erasure.option.tag]\nThe tag is kept.\n\n```\nr[ignored.in-fence]\n```\n";
        let repo = Repo::from_files(&[(README, README_TEXT), ("spec/erasure.md", chapter)]);
        let mut report = Report::default();
        let spec = check(&repo, &mut report);
        assert!(report.messages().is_empty(), "{:?}", report.messages());
        assert_eq!(spec.chapters.len(), 1);
        assert_eq!(spec.chapters[0].title, "Erasure");
        assert_eq!(spec.rules.len(), 1);
        assert_eq!(spec.rules[0].id, "erasure.option.tag");
        assert_eq!(spec.rules[0].line, 3);
    }

    #[test]
    fn rejects_bad_structure() {
        let chapter = "# E\n\nr[erasure.a]\n\nr[erasure.b]\n## Heading\n  r[erasure.c]\nr[Bad]\nx\nr[erasure.a]\ntext\n";
        let repo = Repo::from_files(&[
            (README, README_TEXT),
            ("spec/erasure.md", chapter),
            ("spec/unlisted.md", "# U\n"),
            ("spec/notes.txt", ""),
        ]);
        let mut report = Report::default();
        check(&repo, &mut report);
        assert_eq!(
            report.messages(),
            [
                "error: spec/unlisted.md: every spec chapter is a file directly in spec/, listed under `## Chapters` in spec/README.md",
                "error: spec/notes.txt: spec/ contains only Markdown chapters",
                "error: spec/erasure.md:3: rule `erasure.a` must be followed by the text it marks",
                "error: spec/erasure.md:5: rule `erasure.b` must be followed by the text it marks",
                "error: spec/erasure.md:7: a rule marker is `r[id]` alone at the start of a line",
                "error: spec/erasure.md:8: `Bad` is not a valid rule ID: use dotted kebab-case names, at least two",
                "error: spec/erasure.md:10: rule `erasure.a` is already defined at spec/erasure.md:3",
            ]
        );
    }

    #[test]
    fn fence_modes() {
        let text = "```rust check\n```\n```rust reject\n```\n```rust reject L0001\n```\n```rust prose generated Rust\n```\n```rust\n```\n```rust,ignore\n```\n```rust prose\n```\n```text\n```\n";
        let mut report = Report::default();
        check_fences("spec/a.md", text, false, &mut report);
        let lines: Vec<Option<usize>> = report.errors.iter().map(|f| f.line).collect();
        assert_eq!(lines, [Some(9), Some(11), Some(13)]);
        let mut report = Report::default();
        check_fences("diagnostics/L0001.md", text, true, &mut report);
        assert_eq!(report.errors.len(), 4);
    }
}
