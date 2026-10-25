//! Error-code pages: `diagnostics/Lnnnn.md`, one per code the compiler emits.

use std::collections::BTreeSet;

use crate::markdown;
use crate::repo::Repo;
use crate::report::Report;
use crate::spec;

pub struct Diagnostic {
    pub code: String,
    pub path: String,
    pub title: String,
}

/// `L` followed by four digits.
pub fn is_code(s: &str) -> bool {
    s.len() == 5 && s.starts_with('L') && s[1..].chars().all(|c| c.is_ascii_digit())
}

pub fn check(repo: &Repo, report: &mut Report) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for path in repo.paths.iter().filter(|p| p.starts_with("diagnostics/")) {
        if path == "diagnostics/README.md" {
            continue;
        }
        let code = path
            .strip_prefix("diagnostics/")
            .and_then(|f| f.strip_suffix(".md"))
            .filter(|c| is_code(c));
        let Some(code) = code else {
            report.error(
                path,
                None,
                "expected diagnostics/Lnnnn.md, one page per error code",
            );
            continue;
        };
        let text = repo.text(path).unwrap_or_default();
        let title = markdown::title(text)
            .and_then(|t| t.strip_prefix(code))
            .and_then(|t| t.strip_prefix(": "))
            .map(str::trim)
            .filter(|t| !t.is_empty());
        let Some(title) = title else {
            report.error(
                path,
                Some(1),
                format!("the page starts with `# {code}: <title>`"),
            );
            continue;
        };
        spec::check_fences(path, text, true, report);
        let retired = text.lines().any(|l| l.trim() == "- **Status:** retired");
        let demonstrates = markdown::fences(text)
            .iter()
            .any(|f| f.info.split_whitespace().eq(["rust", "reject", code]));
        if !retired && !demonstrates {
            report.error(
                path,
                None,
                format!("the page needs an example that fails with its code: a `rust reject {code}` fence"),
            );
        }
        out.push(Diagnostic {
            code: code.to_string(),
            path: path.clone(),
            title: title.to_string(),
        });
    }
    out
}

/// The codes that have pages.
pub fn codes(diagnostics: &[Diagnostic]) -> BTreeSet<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_l_and_four_digits() {
        assert!(is_code("L0204"));
        for bad in ["L204", "E0204", "L02045", "l0204", "L02a4"] {
            assert!(!is_code(bad), "{bad}");
        }
    }

    #[test]
    fn pages_need_a_title_and_a_failing_example() {
        let good = "# L0204: arithmetic may overflow\n\n```rust reject L0204\nfn f() {}\n```\n";
        let retired = "# L0003: old\n\n- **Status:** retired\n";
        let repo = Repo::from_files(&[
            ("diagnostics/README.md", "# Index\n"),
            ("diagnostics/L0204.md", good),
            ("diagnostics/L0003.md", retired),
            (
                "diagnostics/L0005.md",
                "# L0005: no example\n\n```rust reject L0006\n```\n",
            ),
            ("diagnostics/L0006.md", "# Wrong title\n"),
            ("diagnostics/E0001.md", ""),
        ]);
        let mut report = Report::default();
        let diagnostics = check(&repo, &mut report);
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(diagnostics[2].title, "arithmetic may overflow");
        assert_eq!(
            report.messages(),
            [
                "error: diagnostics/E0001.md: expected diagnostics/Lnnnn.md, one page per error code",
                "error: diagnostics/L0005.md: the page needs an example that fails with its code: a `rust reject L0005` fence",
                "error: diagnostics/L0006.md:1: the page starts with `# L0006: <title>`",
            ]
        );
    }
}
