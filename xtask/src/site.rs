//! `cargo xtask site`: renders the design documents as a static website. The
//! hand-written landing page in `site/` goes at the root; everything else is
//! staged as an mdBook and built under `docs/`.
//!
//! Staging mirrors the repository's layout, so relative links between
//! published pages keep working. A `README.md` becomes `index.md`. Links to
//! files that are not published point at GitHub instead. Spec rule markers
//! become anchors listing the litmus examples that cite them, and each litmus
//! topic gets a generated page.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use mdbook_driver::MDBook;
use mdbook_driver::config::Config;

use crate::check::{self, Checked};
use crate::litmus::{Example, Outcome};
use crate::markdown;
use crate::repo::{self, Repo};
use crate::report::Report;
use crate::rfc;

const REPOSITORY: &str = "https://github.com/nikhilgarg28/locus";
/// Marks an output directory as ours, so that `--out` never deletes anything else.
const OUTPUT_MARKER: &str = ".locus-site";

/// Hand-written pages, by repository path and published path under `docs/`.
const PAGES: &[(&str, &str)] = &[
    ("site/intro.md", "index.md"),
    ("VISION.md", "VISION.md"),
    ("spec/README.md", "spec/index.md"),
    ("litmus/README.md", "litmus/index.md"),
    ("design/rfcs/README.md", "design/rfcs/index.md"),
    ("design/TRUST.md", "design/TRUST.md"),
    ("diagnostics/README.md", "diagnostics/index.md"),
];

pub fn run(root: &Path, out: &Path) -> Result<(), String> {
    let repo = Repo::load(root)?;
    let mut report = Report::default();
    let checked = check::check_repo(&repo, &mut report);
    report.finish().map_err(|e| {
        format!("{e}: the site is built only from sources that pass `cargo xtask check`")
    })?;
    let stamp = Stamp::current(root);
    let pages = stage(&repo, &checked, &stamp)?;

    let staging = root.join("target/site-src");
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    for (path, text) in &pages {
        write(&staging.join("src").join(path), text.as_bytes())?;
    }
    copy(&root.join("site/book.toml"), &staging.join("book.toml"))?;
    copy(&root.join("site/book.css"), &staging.join("book.css"))?;

    prepare_output(out)?;
    let mut config = Config::from_disk(staging.join("book.toml")).map_err(|e| format!("{e:#}"))?;
    config.build.build_dir = out.join("docs");
    let book = MDBook::load_with_config(&staging, config).map_err(|e| format!("{e:#}"))?;
    book.build().map_err(|e| format!("mdBook: {e:#}"))?;

    let landing = read(&root.join("site/index.html"))?;
    write(&out.join("index.html"), stamp.fill(&landing).as_bytes())?;
    copy(&root.join("site/style.css"), &out.join("style.css"))?;
    write(&out.join(OUTPUT_MARKER), b"")?;
    println!("site built in {}", out.display());
    Ok(())
}

/// Refuses to delete a non-empty directory that this command did not create.
fn prepare_output(out: &Path) -> Result<(), String> {
    if !out.exists() {
        return Ok(());
    }
    let empty = fs::read_dir(out)
        .map_err(|e| format!("{}: {e}", out.display()))?
        .next()
        .is_none();
    if !empty && !out.join(OUTPUT_MARKER).exists() {
        return Err(format!(
            "refusing to replace {}: it was not built by `cargo xtask site`",
            out.display()
        ));
    }
    fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))
}

/// The commit the site is built from.
struct Stamp {
    commit: String,
    date: String,
}

impl Stamp {
    fn current(root: &Path) -> Stamp {
        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        };
        let commit = std::env::var("GITHUB_SHA")
            .ok()
            .or_else(|| git(&["rev-parse", "HEAD"]))
            .unwrap_or_else(|| "unknown".to_string());
        let dirty = git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty());
        let date = git(&["show", "-s", "--format=%cs", "HEAD"]).unwrap_or_default();
        let commit = if dirty {
            format!("{commit}+dirty")
        } else {
            commit
        };
        Stamp { commit, date }
    }

    fn short(&self) -> String {
        match self.commit.split_once('+') {
            Some((sha, rest)) => format!("{}+{rest}", &sha[..7.min(sha.len())]),
            None => self.commit[..7.min(self.commit.len())].to_string(),
        }
    }

    /// Replaces `{{commit}}`, `{{commit-url}}` and `{{date}}`.
    fn fill(&self, text: &str) -> String {
        let sha = self.commit.split('+').next().unwrap_or_default();
        text.replace("{{commit}}", &self.short())
            .replace("{{commit-url}}", &format!("{REPOSITORY}/commit/{sha}"))
            .replace("{{date}}", &self.date)
    }
}

/// Every page of the book, by its path under the book's `src/`.
fn stage(repo: &Repo, checked: &Checked, stamp: &Stamp) -> Result<Vec<(String, String)>, String> {
    let mut targets: BTreeMap<String, String> = PAGES
        .iter()
        .map(|(from, to)| (from.to_string(), to.to_string()))
        .collect();
    for chapter in &checked.spec.chapters {
        targets.insert(chapter.path.clone(), chapter.path.clone());
    }
    for rfc in checked.rfcs.iter().filter(|r| r.path != rfc::TEMPLATE) {
        targets.insert(rfc.path.clone(), rfc.path.clone());
    }
    for diagnostic in &checked.diagnostics {
        targets.insert(diagnostic.path.clone(), diagnostic.path.clone());
    }
    let mut topics: BTreeMap<&str, Vec<&Example>> = BTreeMap::new();
    for example in &checked.examples {
        topics.entry(&example.topic).or_default().push(example);
        targets.insert(
            example.path.clone(),
            format!("{}#{}", topic_page(&example.topic), example.name),
        );
    }
    for topic in topics.keys() {
        targets.insert(format!("litmus/{topic}"), topic_page(topic));
    }
    for (dir, readme) in [
        ("spec", "spec/README.md"),
        ("litmus", "litmus/README.md"),
        ("design/rfcs", "design/rfcs/README.md"),
        ("diagnostics", "diagnostics/README.md"),
    ] {
        targets.insert(dir.to_string(), targets[readme].clone());
    }

    let mut pages = Vec::new();
    for (from, to) in targets.iter().filter(|(from, _)| from.ends_with(".md")) {
        let text = repo
            .text(from)
            .ok_or_else(|| format!("{from} is missing; the site needs it"))?;
        let mut text = relink(repo, &targets, from, to, text);
        if from.starts_with("spec/") || from.starts_with("diagnostics/") {
            text = plain_rust_fences(&text);
        }
        if checked.spec.chapters.iter().any(|c| &c.path == from) {
            text = render_rules(&text, to, &checked.examples);
        }
        text = match from.as_str() {
            "site/intro.md" => stamp.fill(&text),
            "litmus/README.md" => text + &litmus_index(&topics, to),
            "design/rfcs/README.md" => text + &rfc_index(checked),
            "diagnostics/README.md" => text + &diagnostic_index(checked),
            _ => text,
        };
        pages.push((to.clone(), text));
    }
    for (topic, examples) in &topics {
        pages.push((topic_page(topic), topic_page_text(topic, examples, checked)));
    }
    pages.push(("SUMMARY.md".to_string(), summary(checked, &topics)));
    Ok(pages)
}

fn topic_page(topic: &str) -> String {
    format!("litmus/{topic}/index.md")
}

/// Rewrites links in a page published at `to`: links to published sources
/// point at their pages; links to other repository files point at GitHub.
fn relink(
    repo: &Repo,
    targets: &BTreeMap<String, String>,
    from: &str,
    to: &str,
    text: &str,
) -> String {
    markdown::rewrite_links(text, |link| {
        if markdown::is_external(link) || link.starts_with('#') {
            return None;
        }
        let (file, fragment) = match link.split_once('#') {
            Some((file, fragment)) => (file, Some(fragment)),
            None => (link, None),
        };
        let resolved = repo::resolve(from, file)?;
        let resolved = resolved.trim_end_matches('/');
        if let Some(page) = targets.get(resolved) {
            let (page, page_fragment) = match page.split_once('#') {
                Some((page, f)) => (page, Some(f)),
                None => (page.as_str(), None),
            };
            let mut new = repo::relative(to, page);
            if let Some(f) = fragment.or(page_fragment) {
                new = format!("{new}#{f}");
            }
            return Some(new);
        }
        if !repo.exists(resolved) {
            return None;
        }
        let kind = if repo.paths.contains(resolved) {
            "blob"
        } else {
            "tree"
        };
        let fragment = fragment.map(|f| format!("#{f}")).unwrap_or_default();
        Some(format!("{REPOSITORY}/{kind}/main/{resolved}{fragment}"))
    })
}

/// mdBook reads the words after `rust` as its own attributes, so the Locus
/// fence modes (`check`, `reject Lnnnn`, `prose ...`) are dropped for display.
fn plain_rust_fences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let fence_lines: Vec<usize> = markdown::fences(text).iter().map(|f| f.line).collect();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if fence_lines.contains(&(index + 1)) {
            let marker: String = trimmed
                .chars()
                .take_while(|c| *c == '`' || *c == '~')
                .collect();
            let info = trimmed[marker.len()..].trim();
            if info.split_whitespace().next() == Some("rust") {
                out.push_str(&line[..line.len() - trimmed.len()]);
                out.push_str(&marker);
                out.push_str("rust\n");
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Replaces each `r[id]` marker with an anchor and the examples citing it.
fn render_rules(text: &str, page: &str, examples: &[Example]) -> String {
    let mut out = String::with_capacity(text.len());
    for line in markdown::lines(text) {
        let id = match line.in_fence {
            true => None,
            false => crate::spec::marker(line.text.trim_end()),
        };
        let Some(id) = id else {
            out.push_str(line.text);
            out.push('\n');
            continue;
        };
        let citing: Vec<String> = examples
            .iter()
            .filter(|e| e.outcome.is_decided() && e.specs.iter().any(|(_, s)| s == id))
            .map(|e| {
                let href = repo::relative(page, &topic_page(&e.topic).replace(".md", ".html"));
                format!(
                    "<a class=\"{}\" href=\"{href}#{}\">{}/{}</a>",
                    e.outcome.as_str(),
                    e.name,
                    e.topic,
                    e.name
                )
            })
            .collect();
        out.push_str(&format!(
            "<div class=\"rule\" id=\"r-{id}\"><a class=\"rule-id\" href=\"#r-{id}\">{id}</a>\
             <span class=\"rule-examples\">{}</span></div>\n\n",
            citing.join(" ")
        ));
    }
    out
}

fn topic_page_text(topic: &str, examples: &[&Example], checked: &Checked) -> String {
    let page = topic_page(topic);
    let rule_pages: BTreeMap<&str, &str> = checked
        .spec
        .rules
        .iter()
        .map(|r| (r.id.as_str(), r.path.as_str()))
        .collect();
    let mut out = format!(
        "# Litmus: {topic}\n\nExamples from [`litmus/{topic}/`]({REPOSITORY}/tree/main/litmus/{topic}). \
         See [the format](../index.md) for what each outcome means.\n\n\
         | Example | Outcome | Question |\n|---|---|---|\n"
    );
    for e in examples {
        out.push_str(&format!(
            "| [{}](#{}) | {} | {} |\n",
            e.name,
            e.name,
            e.outcome.as_str(),
            escape(e.question.as_deref().unwrap_or(""), true)
        ));
    }
    for e in examples {
        out.push_str(&format!("\n## {}\n\n", e.name));
        let mut facts = vec![format!("**{}**", e.outcome.as_str())];
        let rules: Vec<String> = e
            .specs
            .iter()
            .map(|(_, id)| match rule_pages.get(id.as_str()) {
                Some(chapter) => format!("[{id}]({}#r-{id})", repo::relative(&page, chapter)),
                None => format!("`{id}`"),
            })
            .collect();
        if !rules.is_empty() {
            facts.push(format!("rules: {}", rules.join(", ")));
        }
        if let Some(issue) = e.issue {
            facts.push(format!("issue [#{issue}]({REPOSITORY}/issues/{issue})"));
        }
        if let Some(number) = &e.rfc {
            match checked.rfcs.iter().find(|r| &r.number == number) {
                Some(rfc) => facts.push(format!(
                    "[RFC {number}]({})",
                    repo::relative(&page, &rfc.path)
                )),
                None => facts.push(format!("RFC {number}")),
            }
        }
        facts.push(format!("[source]({REPOSITORY}/blob/main/{})", e.path));
        out.push_str(&facts.join(" · "));
        out.push_str("\n\n");
        if let Some(question) = &e.question {
            out.push_str(&format!("> {}\n\n", escape(question, false)));
        }
        let fence = "`".repeat(longest_backtick_run(&e.source).max(2) + 1);
        out.push_str(&format!("{fence}rust\n{}", e.source));
        if !e.source.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("{fence}\n"));
    }
    out
}

fn litmus_index(topics: &BTreeMap<&str, Vec<&Example>>, page: &str) -> String {
    let mut out = String::from("\n## Examples\n\n");
    if topics.is_empty() {
        out.push_str("There are no examples yet.\n");
        return out;
    }
    out.push_str("| Topic | Accept | Reject | Open |\n|---|---|---|---|\n");
    for (topic, examples) in topics {
        let count = |o| examples.iter().filter(|e| e.outcome == o).count();
        out.push_str(&format!(
            "| [{topic}]({}) | {} | {} | {} |\n",
            repo::relative(page, &topic_page(topic)),
            count(Outcome::Accept),
            count(Outcome::Reject),
            count(Outcome::Open)
        ));
    }
    out.push_str("\n## Open questions\n\n");
    let open: Vec<&&Example> = topics
        .values()
        .flatten()
        .filter(|e| e.outcome == Outcome::Open)
        .collect();
    if open.is_empty() {
        out.push_str("No open questions.\n");
    }
    for e in open {
        out.push_str(&format!(
            "- [{}/{}]({}#{}): {}\n",
            e.topic,
            e.name,
            repo::relative(page, &topic_page(&e.topic)),
            e.name,
            escape(e.question.as_deref().unwrap_or(""), false)
        ));
    }
    out
}

fn rfc_index(checked: &Checked) -> String {
    let rfcs: Vec<_> = checked
        .rfcs
        .iter()
        .filter(|r| r.path != rfc::TEMPLATE)
        .collect();
    let mut out = String::from("\n## Index\n\n");
    if rfcs.is_empty() {
        out.push_str("There are no RFCs yet.\n");
        return out;
    }
    out.push_str("| RFC | Title | Status |\n|---|---|---|\n");
    for rfc in rfcs {
        let file = rfc.path.rsplit('/').next().unwrap_or_default();
        out.push_str(&format!(
            "| [{}]({file}) | {} | {} |\n",
            rfc.number,
            escape(&rfc.title, true),
            rfc.status.as_str()
        ));
    }
    out
}

fn diagnostic_index(checked: &Checked) -> String {
    let mut out = String::from("\n## Index\n\n");
    if checked.diagnostics.is_empty() {
        out.push_str("There are no error codes yet.\n");
        return out;
    }
    out.push_str("| Code | Meaning |\n|---|---|\n");
    for d in &checked.diagnostics {
        out.push_str(&format!(
            "| [{}]({}.md) | {} |\n",
            d.code,
            d.code,
            escape(&d.title, true)
        ));
    }
    out
}

fn summary(checked: &Checked, topics: &BTreeMap<&str, Vec<&Example>>) -> String {
    let mut out = String::from(
        "# Summary\n\n[Locus](index.md)\n\n- [Vision](VISION.md)\n- [Specification](spec/index.md)\n",
    );
    for chapter in &checked.spec.chapters {
        out.push_str(&format!(
            "    - [{}]({})\n",
            summary_title(&chapter.title),
            chapter.path
        ));
    }
    out.push_str("- [Litmus examples](litmus/index.md)\n");
    for topic in topics.keys() {
        out.push_str(&format!("    - [{topic}]({})\n", topic_page(topic)));
    }
    out.push_str("- [RFCs](design/rfcs/index.md)\n");
    for rfc in checked.rfcs.iter().filter(|r| r.path != rfc::TEMPLATE) {
        let title = summary_title(&format!("RFC {}: {}", rfc.number, rfc.title));
        out.push_str(&format!("    - [{title}]({})\n", rfc.path));
    }
    out.push_str("- [Trust model](design/TRUST.md)\n- [Error index](diagnostics/index.md)\n");
    for d in &checked.diagnostics {
        let title = summary_title(&format!("{}: {}", d.code, d.title));
        out.push_str(&format!("    - [{title}]({})\n", d.path));
    }
    out
}

fn summary_title(title: &str) -> String {
    title.replace('[', "\\[").replace(']', "\\]")
}

/// Escapes text for Markdown prose, and for a table cell when `in_table`.
fn escape(text: &str, in_table: bool) -> String {
    let text = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if in_table {
        text.replace('|', "\\|")
    } else {
        text
    }
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn copy(from: &Path, to: &Path) -> Result<(), String> {
    write(to, read(from)?.as_bytes())
}

fn write(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relinks_published_and_unpublished_targets() {
        let repo = Repo::from_files(&[
            ("spec/README.md", ""),
            ("spec/erasure.md", ""),
            ("litmus/t/a.accept.lc", ""),
            ("xtask/src/main.rs", ""),
        ]);
        let targets: BTreeMap<String, String> = [
            ("spec/README.md", "spec/index.md"),
            ("spec/erasure.md", "spec/erasure.md"),
            ("spec", "spec/index.md"),
            ("litmus/t/a.accept.lc", "litmus/t/index.md#a"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let text = "[r](README.md) [c](erasure.md#r-x) [e](../litmus/t/a.accept.lc) \
                    [s](../xtask/src/main.rs) [d](../xtask) [w](https://x.dev) [f](#here)";
        let out = relink(&repo, &targets, "spec/erasure.md", "spec/erasure.md", text);
        assert_eq!(
            out,
            "[r](index.md) [c](erasure.md#r-x) [e](../litmus/t/index.md#a) \
             [s](https://github.com/nikhilgarg28/locus/blob/main/xtask/src/main.rs) \
             [d](https://github.com/nikhilgarg28/locus/tree/main/xtask) [w](https://x.dev) [f](#here)\n"
        );
    }

    #[test]
    fn renders_rule_markers_with_their_examples() {
        let mut report = Report::default();
        let example = crate::litmus::parse(
            "litmus/tag/kept.accept.lc",
            "tag",
            "kept",
            Outcome::Accept,
            "//~ question: q\n//~ spec: erasure.tag\n",
            &mut report,
        );
        let text = "# E\n\nr[erasure.tag]\nThe tag is kept.\n```\nr[erasure.tag]\n```\n";
        let out = render_rules(text, "spec/erasure.md", &[example]);
        assert_eq!(
            out,
            "# E\n\n<div class=\"rule\" id=\"r-erasure.tag\"><a class=\"rule-id\" href=\"#r-erasure.tag\">erasure.tag</a>\
             <span class=\"rule-examples\"><a class=\"accept\" href=\"../litmus/tag/index.html#kept\">tag/kept</a></span></div>\n\n\
             The tag is kept.\n```\nr[erasure.tag]\n```\n"
        );
    }

    #[test]
    fn strips_fence_modes() {
        let text = "```rust reject L0001\nx\n```\n~~~rust prose a b\n~~~\n```text\n```\n";
        assert_eq!(
            plain_rust_fences(text),
            "```rust\nx\n```\n~~~rust\n~~~\n```text\n```\n"
        );
    }
}
