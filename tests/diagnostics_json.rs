//! Process P7: the CLI diagnostic schema and explanations are a versioned API.
use locus::diagnostic::{self, Applicability, Diagnostic, Suggestion};
use locus::source::{SourceBundle, SourceMap, Span};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

#[allow(dead_code)]
#[path = "common/diagnostic_inventory.rs"]
mod inventory;
const ROOT: &str = env!("CARGO_MANIFEST_DIR");
fn root() -> &'static Path {
    Path::new(ROOT)
}
fn bless() -> bool {
    std::env::var("LOCUS_BLESS").is_ok_and(|v| v == "1")
}
fn golden(path: &Path, actual: &str) {
    if bless() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, actual).unwrap();
    }
    assert_eq!(
        fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{}: {e}; run LOCUS_BLESS=1", path.display())),
        actual,
        "{}",
        path.display()
    );
}
fn sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in ["reject", "accept"] {
        for entry in fs::read_dir(root().join("tests/corpus").join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "lc")
                && (directory == "reject" || path.with_extension("stderr").exists())
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
fn cli(directory: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_locus"))
        .current_dir(directory)
        .args(args)
        .env("NO_COLOR", "1")
        .env("LOCUS_PROOFS", "off")
        .output()
        .unwrap()
}
fn workspace() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "locus-json-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn schema(text: &str) {
    let mut process = Command::new("python3")
        .arg(root().join("tools/check_diagnostic_json.py"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    process
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let out = process.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
#[doc = "spec: 1.21:1, 1.21:2"]
fn every_rejected_file_has_json_matching_the_cli_and_schema() {
    let mut all = String::new();
    for path in sources() {
        let relative = path.strip_prefix(root()).unwrap().to_str().unwrap();
        let out = cli(
            root(),
            &["check", relative, "--error-format", "json", "--no-store"],
        );
        let text = String::from_utf8(out.stderr).unwrap();
        assert!(!text.is_empty(), "{relative}: no diagnostics");
        golden(&path.with_extension("jsonl"), &text);
        all.push_str(&text);
    }
    schema(&all);
}
#[test]
#[doc = "spec: 1.21:1"]
fn every_emitted_code_has_an_explanation_and_explain_golden() {
    fn files(path: &Path, out: &mut Vec<PathBuf>) {
        for item in fs::read_dir(path).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                files(&path, out)
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path)
            }
        }
    }
    let mut paths = Vec::new();
    files(&root().join("src"), &mut paths);
    let mut emitted = BTreeSet::new();
    for path in paths {
        if path.ends_with("src/diagnostic/explain.rs") {
            continue;
        }
        emitted.extend(inventory::codes_in(&fs::read_to_string(path).unwrap()));
    }
    let registered: BTreeSet<_> = diagnostic::explain::CODES
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        emitted, registered,
        "explanations must cover exactly the diagnostic registry"
    );
    for code in diagnostic::explain::CODES {
        let explanation = diagnostic::explain::explanation(code).unwrap();
        assert!(
            explanation.contains("Language, as built:"),
            "{code} needs its normative citation"
        );
        assert!(
            explanation.contains("```"),
            "{code} needs a concrete example"
        );
        let out = cli(root(), &["explain", code]);
        assert!(out.status.success());
        assert!(out.stderr.is_empty());
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert_eq!(stdout, explanation);
        golden(
            &root()
                .join("tests/diagnostics/explain")
                .join(format!("{code}.stdout")),
            &stdout,
        );
    }
}
#[test]
#[doc = "spec: 1.21:1"]
fn explanation_citations_name_real_language_paragraphs() {
    let output = Command::new("python3")
        .current_dir(root())
        .args([
            "-c",
            r#"
import sys,re
sys.path.insert(0,'tools')
import spec
from pathlib import Path
ids={p.id for p in spec.inventory(spec.load(Path('atlas.html'))) if p.doc=='language'}
for path in Path('docs/diagnostics').glob('L*.md'):
    citations=re.findall(r'Language, as built: \[([^]]+)\]',path.read_text())
    assert len(citations)==1 and citations[0] in ids,(path,citations)
"#,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[doc = "spec: 1.21:1"]
fn json_escapes_strings_preserves_unicode_and_uses_original_sorted_spans() {
    let mut sources = SourceMap::default();
    let z = sources.add("z.lc", "α\n");
    let a = sources.add("a.lc", "x");
    let bundle = SourceBundle::join(&mut sources, &[z, a]);
    let d = Diagnostic::error(
        "L0102",
        "quoted \"value\" \\ newline\ncontrol\u{0000}",
        Span::new(bundle.file, 4, 5),
    )
    .label(Span::new(bundle.file, 0, 2), "α")
    .note("\t\r\u{001f}")
    .help("literal Unicode α")
    .suggest(Suggestion {
        message: "replace".into(),
        span: Span::new(bundle.file, 4, 5),
        replacement: "\"\\\n".into(),
        applicability: Applicability::MachineApplicable,
    });
    let mapped = bundle.diagnostic(&d);
    let earlier = Diagnostic::warning("L0247", "other", Span::new(z, 0, 2));
    let diagnostics = [earlier, mapped];
    let sorted = diagnostic::sorted(&sources, &diagnostics);
    assert_eq!(sorted[0].code, "L0102");
    let text = sorted
        .iter()
        .map(|d| d.render_json(&sources) + "\n")
        .collect::<String>();
    assert!(text.contains("\\u0000"));
    assert!(text.contains("α"));
    assert!(text.contains("\"column_end\":2"));
    schema(&text);
}
#[test]
#[doc = "spec: 1.21:2"]
fn proof_details_are_structured_without_reading_human_notes() {
    let mut sources = SourceMap::default();
    let file = sources.add(
        "proof.lc",
        "fn fail(n: u8, fact: @(n <= 10)) -> @(n <= 3) { _ }",
    );
    let source = sources.get(file);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success());
    let result = locus::elab::elaborate(source, &parsed.program);
    let d = result
        .diagnostics
        .iter()
        .find(|d| d.code == "L0230")
        .unwrap();
    assert!(d.details.proof.claim.is_some());
    assert!(
        d.details
            .proof
            .facts_considered
            .as_ref()
            .is_some_and(|facts| !facts.is_empty())
    );
    assert!(d.details.proof.counterexample.is_some());
    let mut without_notes = d.clone();
    without_notes.notes.clear();
    let json = without_notes.render_json(&sources);
    assert!(json.contains("\"claim\":\""));
    assert!(!json.contains("\"counterexample\":null"));
    schema(&(json + "\n"));
}
#[test]
#[doc = "spec: 1.21:1"]
fn json_driver_and_resource_failures_are_golden_and_never_plain_text() {
    let dir = workspace();
    fs::write(dir.join("ok.lc"), "fn value() -> u8 { 1 }").unwrap();
    let cases = [
        (
            "usage",
            vec!["check", "ok.lc", "--wrong", "--error-format", "json"],
        ),
        ("missing", vec!["check", "absent.lc", "--error-format=json"]),
        (
            "explain",
            vec!["explain", "L9998", "--error-format", "json"],
        ),
        (
            "library_missing",
            vec![
                "check",
                "ok.lc",
                "--library",
                "absent.lc",
                "--error-format",
                "json",
            ],
        ),
    ];
    for (name, args) in cases {
        let out = cli(&dir, &args);
        assert!(!out.status.success());
        let text = String::from_utf8(out.stderr).unwrap();
        schema(&text);
        golden(
            &root()
                .join("tests/diagnostics/driver")
                .join(format!("{name}.jsonl")),
            &text,
        );
    }
    fs::write(dir.join("ok.lc.proofs"), "invalid").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_locus"))
        .current_dir(&dir)
        .args(["check", "ok.lc", "--error-format=json"])
        .env_remove("LOCUS_PROOFS")
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stderr).unwrap();
    assert!(text.contains("L0402"));
    schema(&text);
    golden(&root().join("tests/diagnostics/driver/store.jsonl"), &text);
    let file = fs::File::create(dir.join("oversized.lc")).unwrap();
    file.set_len(locus::limits::MAX_SOURCE_BYTES as u64 + 1)
        .unwrap();
    let out = cli(&dir, &["check", "oversized.lc", "--error-format=json"]);
    assert!(!out.status.success());
    let text = String::from_utf8(out.stderr).unwrap();
    schema(&text);
    golden(
        &root().join("tests/diagnostics/driver/source_limit.jsonl"),
        &text,
    );
    fs::write(
        dir.join("many.lc"),
        "` ".repeat(locus::limits::MAX_DIAGNOSTICS + 1),
    )
    .unwrap();
    let out = cli(&dir, &["check", "many.lc", "--error-format=json"]);
    assert!(!out.status.success());
    let text = String::from_utf8(out.stderr).unwrap();
    schema(&text);
    assert_eq!(text.lines().count(), locus::limits::MAX_DIAGNOSTICS);
    let terminal = text
        .lines()
        .find(|line| line.contains("L0011"))
        .unwrap()
        .to_owned()
        + "\n";
    golden(
        &root().join("tests/diagnostics/driver/diagnostic_limit.jsonl"),
        &terminal,
    );
    fs::remove_dir_all(dir).unwrap();
}
#[test]
#[doc = "spec: 1.21:1"]
fn json_library_diagnostics_keep_original_paths_and_sort() {
    let dir = workspace();
    fs::write(dir.join("z.lc"), "fn z(n: u8) -> @(n == 1) { _ }").unwrap();
    fs::write(dir.join("a.lc"), "fn a(n: u8) -> @(n == 2) { _ }").unwrap();
    fs::write(dir.join("entry.lc"), "fn main() -> u8 { 0 }").unwrap();
    let out = cli(
        &dir,
        &[
            "check",
            "entry.lc",
            "--library",
            "z.lc",
            "--library",
            "a.lc",
            "--error-format=json",
        ],
    );
    assert!(!out.status.success());
    let text = String::from_utf8(out.stderr).unwrap();
    schema(&text);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("\"file\":\"a.lc\""));
    assert!(lines[1].contains("\"file\":\"z.lc\""));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
#[doc = "spec: 1.21:1"]
fn every_code_has_json_coverage_including_the_inactive_preview_schema() {
    let mut sources = SourceMap::default();
    let file = sources.add("<inactive-preview-schema>", "");
    let gate = Diagnostic::error("L0255", "an unfinished feature requires preview opt-in", Span::new(file,0,0))
        .note("schema fixture only: all current features are stabilized, so this gate has no active source trigger");
    golden(
        &root().join("tests/diagnostics/driver/preview_schema.jsonl"),
        &(gate.render_json(&sources) + "\n"),
    );
    let mut covered = BTreeSet::new();
    let mut paths: Vec<_> = self::sources()
        .into_iter()
        .map(|p| p.with_extension("jsonl"))
        .collect();
    paths.extend(
        fs::read_dir(root().join("tests/diagnostics/driver"))
            .unwrap()
            .map(|entry| entry.unwrap().path()),
    );
    for path in paths {
        let text = fs::read_to_string(path).unwrap();
        covered.extend(inventory::codes_in(&text));
    }
    for code in diagnostic::explain::CODES {
        assert!(covered.contains(*code), "{code} has no JSON golden");
    }
}

#[test]
#[doc = "spec: 1.21:2"]
fn suggested_explicit_forms_recheck_and_are_not_counterexamples() {
    for name in ["definition_opens_only_by_unfold", "equation_needs_rewrite"] {
        let mut text =
            fs::read_to_string(root().join(format!("tests/corpus/reject/{name}.lc"))).unwrap();
        let mut sources = SourceMap::default();
        let file = sources.add(name, &text);
        let parsed = locus::parser::parse(sources.get(file));
        let result = locus::elab::elaborate(sources.get(file), &parsed.program);
        let mut replacements = Vec::new();
        for diagnostic in &result.diagnostics {
            let proof = &diagnostic.details.proof;
            let form = proof
                .suggested_explicit_form
                .clone()
                .expect("checked explicit form");
            assert!(
                proof.counterexample.is_none(),
                "a checked solution rules out a refutation"
            );
            let span = diagnostic
                .labels
                .iter()
                .find(|label| label.primary)
                .unwrap()
                .span;
            replacements.push((span, form));
        }
        replacements.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
        for (span, form) in replacements {
            text.replace_range(span.range(), &form);
        }
        let mut sources = SourceMap::default();
        let file = sources.add(name, text);
        let parsed = locus::parser::parse(sources.get(file));
        assert!(parsed.is_success());
        let checked = locus::elab::elaborate(sources.get(file), &parsed.program);
        assert!(checked.is_success(), "{name}: {:?}", checked.diagnostics);
    }
}
#[test]
#[doc = "spec: 1.21:2"]
fn bounded_explanations_report_depth_and_display_truncation() {
    let deep = std::iter::repeat_n("p", locus::limits::MAX_EXPLANATION_DEPTH + 4)
        .collect::<Vec<_>>()
        .join(" && ");
    let text = format!("logic fn deep(p: Prop, h: @p) -> @({deep}) {{ _ }}");
    let mut sources = SourceMap::default();
    let file = sources.add("deep.lc", text);
    let parsed = locus::parser::parse(sources.get(file));
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let checked = locus::elab::elaborate(sources.get(file), &parsed.program);
    assert!(
        checked
            .diagnostics
            .iter()
            .flat_map(|d| &d.notes)
            .any(|n| n.contains("MAX_EXPLANATION_DEPTH")),
        "{:?}",
        checked.diagnostics
    );
    let facts = (0..locus::limits::MAX_DIAGNOSTIC_FACTS + 2)
        .map(|i| format!("h{i}: @(n <= {})", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!("fn too_many(n: u8, {facts}) -> @(n == 0) {{ _ }}");
    let mut sources = SourceMap::default();
    let file = sources.add("facts.lc", text);
    let parsed = locus::parser::parse(sources.get(file));
    assert!(parsed.is_success());
    let checked = locus::elab::elaborate(sources.get(file), &parsed.program);
    assert!(
        checked
            .diagnostics
            .iter()
            .flat_map(|d| &d.notes)
            .any(|n| n.contains("MAX_DIAGNOSTIC_FACTS")),
        "{:?}",
        checked.diagnostics
    );
}
