//! Golden diagnostics: what an author reads when a file is rejected changes
//! only on purpose.
//!
//! Every `tests/corpus/reject/NAME.lc` has a `NAME.stderr` beside it, holding
//! exactly what `locus check tests/corpus/reject/NAME.lc` writes to stderr
//! from the root of the repository, with colour off. The path in a golden is
//! that relative one, so a golden is the same on every machine. An accepted
//! file, `tests/corpus/accept/NAME.lc`, may have a `NAME.stderr` too: the
//! warnings `check` writes while accepting it.
//!
//! ~~~text
//! LOCUS_BLESS=1 cargo test --test diagnostics_golden -- --nocapture
//! ~~~
//!
//! rewrites the goldens and prints which files changed (`--nocapture` is
//! what lets the list through). The diff is reviewed like code: a golden
//! that reads badly is a bug in the compiler, and fixing it shows up as a
//! change to the golden.
//!
//! The rendering comes from the library, by the steps `check` takes in
//! `src/main.rs`: the parser's diagnostics if the file does not parse, the
//! elaborator's otherwise, each rendered without colour and followed by a
//! newline. A second test runs the binary on every file and compares its
//! stderr with that rendering, so the two cannot drift apart.
//!
//! The meta test lists the error codes the source can emit and fails for each
//! one that no corpus file pins with an `//~ error:` directive, or an
//! `//~ warning:` one for a warning. There is no
//! list of codes excused from this. `UNREACHABLE` is for a code that no source
//! text can reach, which is dead code to remove, and it stays as short as the
//! truth allows.
//!
//! A note for whoever reads a failure here next: S1 adds lexer and parser
//! codes while this file is being written. When it lands, the meta test names
//! the codes that still need a rejected file; write one for each, then bless.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use locus::elab::elaborate;
use locus::parser::parse;
use locus::source::SourceMap;

const REJECT: &str = "tests/corpus/reject";
const ACCEPT: &str = "tests/corpus/accept";

/// Codes in the source that no source text reaches, each with the reason.
/// Such a code is dead code in the compiler: the entry is a request to remove
/// it, not a way to skip writing a rejected file.
const UNREACHABLE: &[(&str, &str)] = &[];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// What `locus check` writes to stderr for a file of this name and text.
fn rendered(name: &str, text: &str) -> String {
    let mut sources = SourceMap::default();
    let file = sources.add(name, text);
    let source = sources.get(file);
    let parsed = parse(source);
    let diagnostics = if parsed.is_success() {
        elaborate(source, &parsed.program).diagnostics
    } else {
        parsed.diagnostics
    };
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}\n", diagnostic.render(&sources, false)))
        .collect()
}

/// The files of a directory of the corpus with this extension, in order,
/// each as its path from the root of the repository.
fn files_with(directory: &str, extension: &str) -> Vec<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(root().join(directory))
        .unwrap_or_else(|error| panic!("{directory}: {error}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|found| found == extension))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            format!(
                "{directory}/{}",
                path.file_name().unwrap().to_string_lossy()
            )
        })
        .collect()
}

/// The files that have a golden, each with whether it is rejected: every
/// rejected file, and the accepted files that have a `.stderr` beside them.
fn files_with_goldens() -> Vec<(String, bool)> {
    let mut files: Vec<(String, bool)> = files_with(REJECT, "lc")
        .into_iter()
        .map(|name| (name, true))
        .collect();
    for name in files_with(ACCEPT, "lc") {
        if root().join(golden_of(&name)).exists() {
            files.push((name, false));
        }
    }
    files
}

fn read(name: &str) -> String {
    std::fs::read_to_string(root().join(name)).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn golden_of(name: &str) -> String {
    format!("{}.stderr", name.strip_suffix(".lc").unwrap())
}

/// A line diff: ` ` before a line both have, `-` before one only the golden
/// has, `+` before one only the rendering has.
fn line_diff(golden: &str, rendering: &str) -> String {
    let old: Vec<&str> = golden.lines().collect();
    let new: Vec<&str> = rendering.lines().collect();
    // common[i][j] is the length of the longest common subsequence of
    // old[i..] and new[j..].
    let mut common = vec![vec![0usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            common[i][j] = if old[i] == new[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut lines = Vec::new();
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            lines.push(format!("  {}", old[i]));
            (i, j) = (i + 1, j + 1);
        } else if j == new.len() || (i < old.len() && common[i + 1][j] >= common[i][j + 1]) {
            lines.push(format!("- {}", old[i]));
            i += 1;
        } else {
            lines.push(format!("+ {}", new[j]));
            j += 1;
        }
    }
    if golden.ends_with('\n') != rendering.ends_with('\n') {
        lines.push("  (they differ in the newline at the end)".into());
    }
    lines.join("\n")
}

#[test]
fn every_rejected_file_renders_as_its_golden() {
    let bless = std::env::var_os("LOCUS_BLESS").is_some_and(|value| value == "1");
    let files: Vec<String> = files_with_goldens()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let mut failures = Vec::new();
    let mut blessed = Vec::new();
    for name in &files {
        let rendering = rendered(name, &read(name));
        let golden_name = golden_of(name);
        let golden = std::fs::read_to_string(root().join(&golden_name)).ok();
        if golden.as_deref() == Some(rendering.as_str()) {
            continue;
        }
        if bless {
            std::fs::write(root().join(&golden_name), &rendering)
                .unwrap_or_else(|error| panic!("{golden_name}: {error}"));
            blessed.push(golden_name);
            continue;
        }
        failures.push(match golden {
            Some(golden) => format!(
                "{name}: the diagnostics differ from {golden_name} (`-` golden, `+` now):\n{}",
                line_diff(&golden, &rendering)
            ),
            None => {
                format!("{name}: there is no {golden_name}; run with LOCUS_BLESS=1 to write it")
            }
        });
    }
    for directory in [REJECT, ACCEPT] {
        for golden_name in files_with(directory, "stderr") {
            let name = format!("{}.lc", golden_name.strip_suffix(".stderr").unwrap());
            if !files.contains(&name) {
                failures.push(format!(
                    "{golden_name}: there is no {name}; delete the golden"
                ));
            }
        }
    }
    if bless {
        println!("LOCUS_BLESS=1: {} golden(s) rewritten", blessed.len());
        for golden_name in &blessed {
            println!("  {golden_name}");
        }
    }
    assert!(
        failures.is_empty(),
        "{} golden diagnostic(s) differ; if the change is meant, run with LOCUS_BLESS=1 and review the diff:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn the_binary_writes_what_the_goldens_are_made_from() {
    let mut failures = Vec::new();
    for (name, rejected) in files_with_goldens() {
        let output = Command::new(env!("CARGO_BIN_EXE_locus"))
            .args(["check", &name])
            .current_dir(root())
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        let rendering = rendered(&name, &read(&name));
        let (expected_status, kind) = if rejected {
            (1, "a rejected file")
        } else {
            (0, "an accepted file")
        };
        if output.status.code() != Some(expected_status) {
            failures.push(format!(
                "{name}: `locus check` exited with {:?}, and {kind} exits with {expected_status}",
                output.status.code()
            ));
        }
        if rejected && !output.stdout.is_empty() {
            failures.push(format!("{name}: `locus check` wrote to stdout"));
        }
        if stderr != rendering {
            failures.push(format!(
                "{name}: `locus check` and this test render differently (`-` test, `+` binary):\n{}",
                line_diff(&rendering, &stderr)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failure(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

/// Every `"L0123"` in a Rust source text: a string literal that is exactly an
/// error code.
///
/// This is a scan of the text and knows nothing of Rust. It misses a code
/// that is not written whole in one literal under `src/`: one assembled with
/// `format!` or `concat!`, or one that comes from another crate. It also
/// takes any literal of this shape for a code, in a comment or in a test as
/// much as in a call to `Diagnostic::error`; that errs towards asking for a
/// rejected file, which is the safe side.
fn codes_in(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut codes = BTreeSet::new();
    for start in 0..bytes.len().saturating_sub(6) {
        let window = &bytes[start..start + 7];
        if window[0] == b'"'
            && window[1] == b'L'
            && window[2..6].iter().all(u8::is_ascii_digit)
            && window[6] == b'"'
        {
            codes.insert(text[start + 1..start + 6].to_string());
        }
    }
    codes
}

/// The codes of the `//~ error:` and `//~ warning:` directives of a corpus
/// file, read as `tests/corpus.rs` reads them: `^`s may follow `//~`, and
/// the code is the first word after the key. That test rejects a malformed
/// directive; this one only collects.
fn codes_pinned_in(text: &str) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    for line in text.lines() {
        let Some((_, rest)) = line.split_once("//~") else {
            continue;
        };
        let Some((key, value)) = rest.trim_start_matches('^').split_once(':') else {
            continue;
        };
        if matches!(key.trim(), "error" | "warning") {
            codes.extend(value.split_whitespace().next().map(str::to_string));
        }
    }
    codes
}

fn rust_files_under(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files_under(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn every_error_code_in_the_source_has_a_rejected_file() {
    let mut sources = Vec::new();
    rust_files_under(&root().join("src"), &mut sources);
    let mut emitted = BTreeSet::new();
    for path in &sources {
        emitted.extend(codes_in(&std::fs::read_to_string(path).unwrap()));
    }
    let mut pinned = BTreeSet::new();
    for directory in [REJECT, ACCEPT] {
        for name in files_with(directory, "lc") {
            pinned.extend(codes_pinned_in(&read(&name)));
        }
    }
    assert!(!emitted.is_empty(), "the scan of `src/` found no codes");

    let mut failures = Vec::new();
    for code in emitted.difference(&pinned) {
        match UNREACHABLE.iter().find(|(excused, _)| excused == code) {
            Some(_) => {}
            None => failures.push(format!(
                "{code} is in the source, and no file in {REJECT} has `//~ error: {code}` (or, for a warning, no file in {ACCEPT} has `//~ warning: {code}`)"
            )),
        }
    }
    for code in pinned.difference(&emitted) {
        failures.push(format!(
            "{code} is pinned by a corpus file, and the scan of `src/` did not find it"
        ));
    }
    for (code, why) in UNREACHABLE {
        if pinned.contains(*code) {
            failures.push(format!(
                "{code} is listed as unreachable ({why}), and a rejected file reaches it"
            ));
        }
        if !emitted.contains(*code) {
            failures.push(format!(
                "{code} is listed as unreachable ({why}), and it is no longer in the source"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failure(s); write the smallest rejected file for each code, then run with LOCUS_BLESS=1:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// The test, tested on itself.

#[test]
fn a_diff_marks_the_lines_that_differ_and_keeps_the_rest() {
    let golden = "error[L0204]: unknown name `missing`\n  |\n2 |     missing\n";
    let rendering = "error[L0204]: unknown name `missing`\n  |\n2 |     missing\n  = note: new\n";
    assert_eq!(
        line_diff(golden, golden),
        golden
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        line_diff(golden, rendering),
        "  error[L0204]: unknown name `missing`\n    |\n  2 |     missing\n+   = note: new"
    );
    assert_eq!(line_diff("a\nb\nc\n", "a\nx\nc\n"), "  a\n- b\n+ x\n  c");
    assert_eq!(
        line_diff("a\n", "a"),
        "  a\n  (they differ in the newline at the end)"
    );
}

#[test]
fn only_a_whole_literal_is_a_code() {
    let text =
        r#"Diagnostic::error("L0204", "unknown name L0205", span); fail("L0290","L02999", "L029")"#;
    assert_eq!(
        codes_in(text).into_iter().collect::<Vec<_>>(),
        ["L0204", "L0290"]
    );
    assert!(codes_in("\"L0").is_empty());
}

#[test]
fn pinned_codes_are_read_as_the_corpus_runner_reads_them() {
    let text = "fn f() -> u8 { //~ error: L0203\n    g() //~ error: L0204 unknown function\n    //~^^ error: L0220\n}\n//~ run: f() => 1\n// error: L0230\n";
    assert_eq!(
        codes_pinned_in(text).into_iter().collect::<Vec<_>>(),
        ["L0203", "L0204", "L0220"]
    );
}

#[test]
fn the_rendering_names_the_file_as_it_was_given() {
    let text = "fn first(n: u8) -> u8 {\n    missing\n}\n";
    let shown = rendered("tests/corpus/reject/example.lc", text);
    assert!(shown.starts_with("error[L0204]: unknown name `missing`\n"));
    assert!(shown.contains(" --> tests/corpus/reject/example.lc:2:5\n"));
    assert!(shown.ends_with('\n'));
    assert!(!shown.contains('\u{1b}'), "colour is off");
}
