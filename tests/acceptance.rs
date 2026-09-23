//! Acceptance (R4): the exit criteria of the Build plan, "What done means",
//! one test each, named after the criterion. Each test does the part of its
//! criterion that fits the fast gate and prints one line beginning
//! `criterion:` with what it measured; `tools/check.sh --extended` runs the
//! suite in release under `LOCUS_EXTENDED=1`, where the randomized tests of
//! the other files run their long forms, and prints one line per criterion
//! from what the tests printed. Three criteria rest on those long runs and
//! are pinned here by the numbers the runs are configured with:
//!
//! - "What is checked is what runs" is `tests/random_programs.rs`, 10,000
//!   programs under `LOCUS_EXTENDED`;
//! - "The trusted base is written down and tested as such" is
//!   `tests/kernel_soundness.rs` (mutants), `tests/kernel_ops.rs`,
//!   `tests/kernel_machine.rs`, and `tests/kernel_lemmas.rs` (the models
//!   against Rust), and the contract test in `tests/kernel_int.rs`;
//! - "The parser is robust, and total for stated reasons" is
//!   `tests/parser_fuzz.rs` at a hundred times its default counts.
//!
//! The target examples are run here through the corpus runner of
//! `tests/common/corpus.rs`, which `tests/corpus.rs` runs over every file.

#[path = "common/corpus.rs"]
mod runner;

#[path = "common/diagnostic_inventory.rs"]
mod diagnostic_inventory;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use locus::diagnostic::Diagnostic;
use locus::elab::elaborate_with;
use locus::erased::print_module;
use locus::kernel::{
    Axiom, CmpOp, Context, Definitions, HypId, HypRef, KernelError, MachineInt, Op, Proof, Term,
    Type, check_proof, infer_proof,
};
use locus::parser::parse;
use locus::source::SourceMap;
use runner::{TIMEOUT, compile_and_compare, examine, expects_rejection, files_in, is_parse_only};

const TARGET: [&str; 3] = ["lock", "midpoint", "percent"];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn target(name: &str) -> PathBuf {
    root()
        .join("tests/corpus/target")
        .join(format!("{name}.lc"))
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn locus() -> Command {
    Command::new(env!("CARGO_BIN_EXE_locus"))
}

/// A scratch directory of its own, emptied first.
fn scratch(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("acceptance_{name}"));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn rustc() -> Command {
    Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
}

/// The diagnostics of a text, parsed and elaborated in this process.
fn diagnostics_of(name: &str, text: &str) -> (bool, Vec<Diagnostic>) {
    let mut sources = SourceMap::default();
    let file = sources.add(name, text);
    let source = sources.get(file);
    let parsed = parse(source);
    if !parsed.is_success() {
        return (false, parsed.diagnostics);
    }
    let mut options = locus::elab::Options::default();
    for name in ["logical-data", "heap-views"] {
        if text.contains(&format!("//~ preview: {name}")) {
            options.previews.enable(name).unwrap();
        }
    }
    let elaborated = locus::elab::elaborate_with_options(source, &parsed.program, &options);
    (elaborated.is_success(), elaborated.diagnostics)
}

/// The lines of `stdout` and `stderr` of the compiler run with these
/// arguments, and its exit code.
fn run_locus(arguments: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String, String) {
    let mut command = locus();
    command.args(arguments);
    if !arguments.contains(&"--preview") {
        for argument in arguments.iter().filter(|arg| arg.ends_with(".lc")) {
            if let Ok(text) = std::fs::read_to_string(argument) {
                for name in ["logical-data", "heap-views"] {
                    if text.contains(&format!("//~ preview: {name}")) {
                        command.args(["--preview", name]);
                    }
                }
            }
        }
    }
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// Every source file under `src`.
fn source_files() -> Vec<PathBuf> {
    fn walk(directory: &Path, found: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(&root().join("src"), &mut found);
    found
}

/// A Markdown document or the assembled language manual, through the compatibility CLI.
fn atlas_document(id: &str) -> String {
    let output = Command::new("python3")
        .current_dir(root())
        .args(["tools/atlas.py", "show", id])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Whether `name` occurs in `text` as a whole identifier.
fn mentions(text: &str, name: &str) -> bool {
    let part = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(part)
            && !text[at + name.len()..].chars().next().is_some_and(part)
    })
}

// --- The criteria -----------------------------------------------------------------

#[test]
fn the_target_examples_run() {
    // The 32-bit lock, the midpoint, and the protected type check, run in
    // both interpreters in both overflow modes, compile under rustc with
    // warnings denied in both builds, and the three agree on every run
    // line, panics included. Percent answers with an enum of its own.
    let files: Vec<_> = files_in("tests/corpus/target")
        .into_iter()
        .filter(|(name, _)| {
            TARGET
                .iter()
                .any(|target| name.ends_with(&format!("/{target}.lc")))
        })
        .collect();
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        TARGET.map(|name| format!("tests/corpus/target/{name}.lc"))
    );
    let mut failures = Vec::new();
    let mut inconclusive = Vec::new();
    let mut compiled = Vec::new();
    let mut runs = 0;
    let mut panics = 0;
    for (name, text) in &files {
        assert!(!is_parse_only(text), "{name} is only parsed");
        assert!(!expects_rejection(text), "{name} expects an error");
        let examined = examine(name, text);
        failures.extend(examined.failures);
        inconclusive.extend(examined.inconclusive);
        let unit = examined
            .compiled
            .unwrap_or_else(|| panic!("{name} was not accepted"));
        runs += unit.runs.len();
        panics += unit
            .runs
            .iter()
            .filter(|run| matches!(run.expected, runner::Expected::Panic(_)))
            .count();
        compiled.push(unit);
    }
    assert!(runs >= 16, "{runs} run lines");
    assert_eq!(panics, 0, "the target APIs all promise no_panic");
    let report = compile_and_compare(&compiled, "locus_acceptance_target", TIMEOUT);
    failures.extend(report.failures);
    inconclusive.extend(report.inconclusive);
    let listed = |list: &[runner::Failure]| {
        list.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(failures.is_empty(), "{}", listed(&failures));
    assert!(inconclusive.is_empty(), "{}", listed(&inconclusive));
    let percent = read(&target("percent"));
    let code: Vec<&str> = percent
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    assert!(code.iter().any(|line| line.contains("Option<Percent>")));
    println!(
        "criterion: the target examples run: 3 files, {runs} run lines ({panics} that panic) \
         agree in the check-IR interpreter, the erased interpreter, and compiled Rust, with \
         overflow checks on and off"
    );
}

#[test]
fn the_proofs_are_the_ones_predicted() {
    // `check --stats` classifies the obligations of the target files by the
    // tier that filled them. Target examples predicts, over the lock and
    // the midpoint, three exact, four computed, and seven arithmetic; the
    // compiler's counts differ, and the difference is recorded there as a
    // finding rather than adjusted here: `0 <= 3` and `2 != 0` on literals
    // are decided by evaluation, which the prediction folds into computed;
    // an overflow row has two premises, so each `+` and `-` adds an
    // arithmetic obligation; and `ok = still` and `(next, bounded)` are
    // typed assignments and values, not holes, so they count as nothing.
    let counts = |name: &str| {
        let path = target(name);
        let mut arguments = vec!["check", path.to_str().unwrap(), "--stats"];
        if name == "percent"
            && locus::preview::Feature::LogicalData.status() == locus::preview::Status::Preview
        {
            arguments.extend(["--preview", "logical-data"]);
        }
        let (code, stdout, stderr) = run_locus(&arguments, &[("LOCUS_PROOFS", "off")]);
        assert_eq!(code, Some(0), "{stderr}");
        stdout
            .lines()
            .find(|line| line.starts_with("obligations: "))
            .unwrap_or_else(|| panic!("no obligations line for {name}:\n{stdout}"))
            .to_string()
    };
    let lock = counts("lock");
    let midpoint = counts("midpoint");
    let percent = counts("percent");
    assert_eq!(
        lock,
        "obligations: 11 (3 computed, 2 evaluation, 6 arithmetic)"
    );
    assert_eq!(midpoint, "obligations: 6 (1 evaluation, 5 arithmetic)");
    assert_eq!(percent, "obligations: 1 (1 computed)");
    println!(
        "criterion: the proofs are the ones predicted: lock {}; midpoint {}; percent {}; \
         Target examples predicts 3 exact, 4 computed, 7 arithmetic over the lock and the \
         midpoint, and the difference is recorded there",
        &lock["obligations: ".len()..],
        &midpoint["obligations: ".len()..],
        &percent["obligations: ".len()..]
    );
}

/// `locus build` on the target files, compiled by rustc as a library under
/// `-D warnings`: the crate's directory and its rlib.
fn generated_target_crate(name: &str) -> (PathBuf, PathBuf) {
    let workspace = scratch(name);
    let out = workspace.join("generated");
    // The boundary regression also exercises setters and panic recovery;
    // these are deliberately separate from the exact Atlas examples.
    let lock = workspace.join("lock.lc");
    let percent = workspace.join("percent.lc");
    std::fs::write(
        &lock,
        read(&target("lock")).replace("derive(Clone, Copy)", "derive(Clone, Copy, Debug)"),
    )
    .unwrap();
    std::fs::write(
        &percent,
        read(&root().join("tests/corpus/accept/percent_updates.lc")),
    )
    .unwrap();
    let (code, _, stderr) = run_locus(
        &[
            "build",
            lock.to_str().unwrap(),
            percent.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--name",
            "generated",
        ],
        &[],
    );
    assert_eq!(code, Some(0), "locus build failed:\n{stderr}");
    let rlib = out.join("libgenerated.rlib");
    let output = rustc()
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--crate-name",
            "generated",
            "-D",
            "warnings",
            "-o",
        ])
        .arg(&rlib)
        .arg(out.join("src").join("lib.rs"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc rejected the generated crate:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (out, rlib)
}

/// Hand-written Rust compiled against the generated crate: the binary, or
/// rustc's error codes when it refused.
fn caller(directory: &Path, name: &str, source: &str, rlib: &Path) -> Result<PathBuf, Vec<String>> {
    let path = directory.join(format!("{name}.rs"));
    std::fs::write(&path, source).unwrap();
    let binary = directory.join(name);
    let output = rustc()
        .args(["--edition", "2024", "--crate-type", "bin", "-o"])
        .arg(&binary)
        .arg("--extern")
        .arg(format!("generated={}", rlib.display()))
        .arg(&path)
        .output()
        .unwrap();
    if output.status.success() {
        return Ok(binary);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("error[")?;
            Some(rest[..rest.find(']')?].to_string())
        })
        .collect())
}

#[test]
fn a_rust_caller_is_held_at_the_boundary() {
    // Against the generated crate of the target lock and Percent: what is
    // plain `pub` is called; what takes evidence is not reachable; the
    // marker cannot be named; a protected struct cannot be built; the
    // marker replay attack, a marker obtained honestly handed to a function
    // that wants evidence of something else, fails to compile; and a value
    // a Rust caller still holds after catching a panic satisfies its
    // invariant (LOC-191). rustc gives each verdict.
    let (directory, rlib) = generated_target_crate("boundary");
    let mut verdicts = Vec::new();
    let mut refused = |name: &str, source: &str, expected: &[&str]| {
        let codes = caller(&directory, name, source, &rlib)
            .err()
            .unwrap_or_else(|| panic!("rustc accepted {name}"));
        assert_eq!(codes, expected, "{name}");
        verdicts.push(format!("{name} {}", codes.join("+")));
    };
    refused(
        "calls_new",
        "fn main() {\n    let percent = generated::percent::Percent::new(5, todo!());\n    println!(\"{}\", percent.value());\n}\n",
        &["E0624"],
    );
    refused(
        "calls_step",
        "fn main() {\n    let (lock, ok) = generated::lock::run(0, 9);\n    let (next, _) = generated::lock::step(lock, ok, generated::lock::Event::Wrong);\n    println!(\"{next:?}\");\n}\n",
        &["E0603"],
    );
    refused(
        "replays_the_marker",
        "fn main() {\n    let (_, honest) = generated::lock::run(0, 9);\n    let (left, _) = generated::lock::remaining(200, honest);\n    println!(\"{left}\");\n}\n",
        &["E0603"],
    );
    refused(
        "names_the_marker",
        "fn main() {\n    let _: generated::Erased = generated::Erased;\n}\n",
        &["E0603"],
    );
    refused(
        "builds_the_marker",
        "fn main() {\n    let _ = generated::Erased { _private: () };\n}\n",
        &["E0451"],
    );
    refused(
        "builds_percent",
        "fn main() {\n    let percent = generated::percent::Percent { value: 200, in_range: todo!() };\n    println!(\"{}\", percent.value());\n}\n",
        &["E0451"],
    );
    refused(
        "reads_percent",
        "fn main() {\n    if let generated::percent::Checked::Valid(percent) = generated::percent::Percent::checked(5) {\n        println!(\"{}\", percent.value);\n    }\n}\n",
        &["E0616"],
    );
    let binary = caller(
        &directory,
        "uses_the_exports",
        "use generated::percent::{Checked, Percent};\nfn main() {\n    std::panic::set_hook(Box::new(|_| {}));\n    let (lock, _) = generated::lock::run(300, 9);\n    println!(\"{lock:?}\");\n    for value in [42, 100, 101] {\n        match Percent::checked(value) {\n            Checked::Valid(percent) => println!(\"valid {}\", percent.value()),\n            Checked::Invalid => println!(\"invalid\"),\n        }\n    }\n    let mut percent = match Percent::checked(3) {\n        Checked::Valid(percent) => percent,\n        Checked::Invalid => unreachable!(),\n    };\n    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| percent.set_twice(5, 200)));\n    println!(\"panicked {} value {}\", outcome.is_err(), percent.value());\n    assert!(percent.value() <= 100);\n}\n",
        &rlib,
    )
    .unwrap_or_else(|codes| panic!("rustc refused the caller of the exports: {codes:?}"));
    let output = Command::new(binary).output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Lock { failures: 3, open: false }\nvalid 42\nvalid 100\ninvalid\npanicked true value 5\n"
    );
    println!(
        "criterion: a Rust caller is held at the boundary: the plain pub exports run from Rust; \
         refused by rustc: {}; a caught panic leaves a valid Percent",
        verdicts.join(", ")
    );
}

#[test]
#[doc = "spec: 1.21:1"]
fn an_author_is_told_why() {
    // Each target example with a hypothesis removed: the first diagnostic
    // names the obligation, states the claim as computed, says which facts
    // were considered, and gives the counterexample the arithmetic procedure
    // found. And every error code in the source has a rejected file that
    // pins it and a golden rendering beside that file.
    type Edits = Vec<(&'static str, &'static str)>;
    let removed: [(&str, Edits, &str, &str); 3] = [
        (
            "lock",
            vec![
                ("bounded: @within_limit(failures as Int)", ""),
                ("(failures: u32, )", "(failures: u32)"),
                (
                    "    let small = logic {\n        let within_limit::Bounds @ small = bounded;\n        small\n    };                                                                        // model(failures) <= 3\n",
                    "",
                ),
            ],
            "L0235",
            "`-` on `u32` may overflow, and `remaining` promises no_panic",
        ),
        (
            "midpoint",
            vec![(", ordered: @((lo as Int) <= (hi as Int))", "")],
            "L0235",
            "`-` on `u32` may overflow, and `midpoint` promises no_panic",
        ),
        (
            "percent",
            vec![(
                "if value <= 100 { Some(Percent::new(value, prove!((value as Int) <= 100))) } else { None }",
                "Some(Percent::new(value, prove!((value as Int) <= 100)))",
            )],
            "L0230",
            "cannot show `value <= 100`",
        ),
    ];
    let mut told = Vec::new();
    for (name, edits, code, message) in removed {
        let mut text = read(&target(name));
        for (old, new) in edits {
            assert_eq!(text.matches(old).count(), 1, "{name}: {old}");
            text = text.replace(old, new);
        }
        let (accepted, diagnostics) = diagnostics_of(&format!("{name}.lc"), &text);
        assert!(!accepted, "{name} was accepted without its hypothesis");
        let first = &diagnostics[0];
        assert_eq!(first.code, code, "{name}: {first:#?}");
        assert_eq!(first.message, message, "{name}");
        let notes = first.notes.join("\n");
        // The claim as computed: the bound the row asks for, or the claim
        // itself when it is the obligation.
        assert!(
            notes.contains("must be at least 0") || first.message.starts_with("cannot show"),
            "{name}: {notes}"
        );
        // The facts considered.
        assert!(notes.contains("known here"), "{name}: {notes}");
        // The counterexample.
        assert!(
            notes.contains("it fails when")
                && notes.contains("which the arithmetic facts known here allow"),
            "{name}: {notes}"
        );
        let counterexample = first
            .notes
            .iter()
            .find_map(|note| note.strip_prefix("it fails when "))
            .unwrap();
        told.push(format!(
            "{name} {code} ({})",
            counterexample.split(", which").next().unwrap()
        ));
    }

    // Every error code has a rejected file and a golden rendering.
    let mut codes = Vec::new();
    for path in source_files() {
        for code in diagnostic_inventory::source_codes(&path, &read(&path)) {
            if !codes.contains(&code) {
                codes.push(code);
            }
        }
    }
    codes.sort();
    let mut pinned = Vec::new();
    let mut goldens = 0;
    for directory in ["tests/corpus/reject", "tests/corpus/accept"] {
        for (name, text) in files_in(directory) {
            for line in text.lines() {
                if let Some(rest) = line.split("//~").nth(1)
                    && let Some(rest) = rest
                        .trim_start()
                        .trim_start_matches('^')
                        .trim_start()
                        .strip_prefix("error: ")
                        .or_else(|| {
                            rest.trim_start()
                                .trim_start_matches('^')
                                .trim_start()
                                .strip_prefix("warning: ")
                        })
                {
                    let code = rest
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    if !pinned.contains(&code) {
                        pinned.push(code);
                    }
                }
            }
            if directory == "tests/corpus/reject" {
                let golden = root().join(name.replace(".lc", ".stderr"));
                assert!(golden.exists(), "{name} has no golden rendering beside it");
                goldens += 1;
            }
        }
    }
    let unpinned: Vec<&String> = codes.iter().filter(|code| !pinned.contains(code)).collect();
    assert!(
        unpinned.is_empty(),
        "codes with no rejected file: {unpinned:?}"
    );
    println!(
        "criterion: an author is told why: with a hypothesis removed, {}; {} error codes, each \
         pinned by a corpus file, {goldens} rejected files each with a golden rendering",
        told.join("; "),
        codes.len()
    );
}

#[test]
#[doc = "spec: 1.20:1"]
fn a_crate_can_be_checked_by_the_kernel_alone() {
    // Every example and target file checks under `--locked` with the
    // committed `Locus.lock` of its directory, which runs no search, and
    // checks with the search disabled altogether, which is what a proof
    // written by a weaker search amounts to. The lockfile is left as it
    // was.
    let directory = scratch("locked");
    let mut checked = 0;
    for source in ["examples", "tests/corpus/target"] {
        let lock = root().join(source).join("Locus.lock");
        assert!(lock.exists(), "{source} has no committed Locus.lock");
        let copies = directory.join(source.replace('/', "_"));
        std::fs::create_dir_all(&copies).unwrap();
        let copied_lock = copies.join("Locus.lock");
        std::fs::copy(&lock, &copied_lock).unwrap();
        let before = read(&copied_lock);
        for (name, _) in files_in(source) {
            let path = root().join(&name);
            let copy = copies.join(path.file_name().unwrap());
            std::fs::copy(&path, &copy).unwrap();
            let (code, stdout, stderr) = run_locus(
                &["check", copy.to_str().unwrap(), "--locked", "--stats"],
                &[],
            );
            assert_eq!(code, Some(0), "{name} --locked:\n{stderr}");
            let used = stdout
                .lines()
                .find(|line| line.starts_with("Locus.lock: "))
                .unwrap_or_else(|| panic!("{name}: no Locus.lock line:\n{stdout}"));
            assert!(
                used.contains("0 found and recorded") && used.contains(", 0 searched"),
                "{name}: {used}"
            );
            let (code, _, stderr) = run_locus(
                &["check", copy.to_str().unwrap(), "--locked"],
                &[("LOCUS_SEARCH", "none")],
            );
            assert_eq!(code, Some(0), "{name} with no search:\n{stderr}");
            assert_eq!(read(&copied_lock), before, "{name}: the lockfile changed");
            checked += 1;
        }
    }
    println!(
        "criterion: a crate can be checked by the kernel alone: {checked} files check under \
         --locked with their committed proofs, 0 searched, and with the search disabled"
    );
}

#[test]
fn checking_is_deterministic() {
    // Checking a file twice gives byte-identical proofs, diagnostics, and
    // Rust. No hole is filled by search that bridges claims: evidence of
    // another claim is rejected. And no limit in the checker is a clock.
    let directory = scratch("deterministic");
    let mut files = 0;
    for name in TARGET {
        let path = target(name);
        let mut runs = Vec::new();
        for pass in 0..2 {
            // A directory of its own, so that the lockfile is the file's alone.
            let copies = directory.join(format!("{name}_{pass}"));
            std::fs::create_dir_all(&copies).unwrap();
            let copy = copies.join(format!("{name}.lc"));
            std::fs::copy(&path, &copy).unwrap();
            let (code, holes, stderr) =
                run_locus(&["check", copy.to_str().unwrap(), "--holes"], &[]);
            assert_eq!(code, Some(0), "{stderr}");
            let (code, rust, _) = run_locus(&["rust", copy.to_str().unwrap()], &[]);
            assert_eq!(code, Some(0));
            let proofs = read(&copies.join("Locus.lock"));
            // The listing of holes carries timings, which are the one thing
            // in it that may differ between runs.
            let holes: Vec<String> = holes
                .lines()
                .map(|line| line.replace(copy.to_str().unwrap(), "FILE"))
                .map(|line| match line.rfind(", ") {
                    Some(cut) if line.ends_with(" us)") => format!("{})", &line[..cut]),
                    _ => line,
                })
                .collect();
            runs.push((holes, stderr, rust, proofs));
        }
        assert_eq!(runs[0], runs[1], "{name} checked differently twice");
        // Independent checks of the same relative source path produce the
        // same lockfile. Obligation keys do not depend on the parent path.
        assert!(!runs[0].3.is_empty());
        files += 1;
    }
    let evidence = read(&root().join("tests/corpus/reject/evidence_of_another_claim.lc"));
    let (accepted, diagnostics) = diagnostics_of("evidence_of_another_claim.lc", &evidence);
    assert!(!accepted);
    assert!(
        diagnostics.iter().any(|d| d.code == "L0230"),
        "{diagnostics:#?}"
    );
    let mut clocks = Vec::new();
    for path in source_files() {
        let text = read(&path);
        let relative = path
            .strip_prefix(root())
            .unwrap()
            .to_string_lossy()
            .into_owned();
        // The elaborator measures itself for `--stats` and `--holes`; the
        // checker's limits are counts.
        let measuring = relative.starts_with("src/elab/") || relative == "src/main.rs";
        if text.contains("std::time") && !measuring {
            clocks.push(relative.clone());
        }
        if measuring
            && (text.contains("Duration")
                || text.contains("elapsed() >")
                || text.contains("elapsed() <"))
            && relative != "src/main.rs"
        {
            clocks.push(relative);
        }
    }
    assert!(clocks.is_empty(), "a clock in the checker: {clocks:?}");
    println!(
        "criterion: checking is deterministic: {files} files give the same holes, diagnostics, \
         Rust, and lockfile twice; evidence of another claim is rejected; no clock in the \
         checker"
    );
}

#[test]
fn locus_is_never_more_permissive_than_rustc() {
    // Every accepted target program compiles without warnings (the corpus
    // test does the same for every accepted file), and every program the
    // corpus rejects for a move is rejected by rustc too when its Rust is
    // printed with the move analysis skipped. The aliasing rejections have
    // the same oracle in `tests/references.rs`, shape by shape, where the
    // printed Rust is rewritten into the rejected form, since lowering
    // refuses an overlap whatever the analysis says.
    let directory = scratch("rustc_oracle");
    let oracle: [(&str, &str); 4] = [
        ("use_after_move", "E0382"),
        ("moved_in_loop", "E0382"),
        ("moved_in_one_arm", "E0382"),
        ("move_out_of_reference", "E0507"),
    ];
    let mut agreed = Vec::new();
    for (name, expected) in oracle {
        let text = read(&root().join(format!("tests/corpus/reject/{name}.lc")));
        let mut sources = SourceMap::default();
        let file = sources.add(name, &text);
        let source = sources.get(file);
        let parsed = parse(source);
        assert!(parsed.is_success(), "{name}: {:#?}", parsed.diagnostics);
        let strict = elaborate_with(source, &parsed.program, true);
        assert!(!strict.is_success(), "{name} was accepted");
        let lenient = elaborate_with(source, &parsed.program, false);
        assert!(
            lenient.is_success(),
            "{name} is rejected for another reason too: {:#?}",
            lenient.diagnostics
        );
        let rust = print_module(lenient.session.erased());
        let source_path = directory.join(format!("{name}.rs"));
        std::fs::write(&source_path, &rust).unwrap();
        let output = rustc()
            .args([
                "--edition",
                "2021",
                "--crate-type",
                "lib",
                "-D",
                "warnings",
                "-o",
            ])
            .arg(directory.join(format!("lib{name}.rlib")))
            .arg(&source_path)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name}: rustc accepted the Rust");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!("error[{expected}]")),
            "{name}: rustc rejected the Rust for another reason:\n{stderr}"
        );
        agreed.push(format!("{name} {expected}"));
    }
    println!(
        "criterion: Locus is never more permissive than rustc: {} programs rejected for a move \
         are rejected by rustc with the same code ({}); aliasing has the same oracle in \
         tests/references.rs; every accepted file compiles under -D warnings in the corpus test",
        agreed.len(),
        agreed.join(", ")
    );
}

/// A constant of a test file, `const NAME: T = value;`, as text.
fn constant_in(file: &str, name: &str) -> String {
    let text = read(&root().join("tests").join(file));
    let key = format!("const {name}: ");
    let start = text
        .find(&key)
        .unwrap_or_else(|| panic!("{file} has no {name}"));
    let rest = &text[start + key.len()..];
    let value = &rest[rest.find('=').unwrap() + 1..rest.find(';').unwrap()];
    value.trim().replace('_', "")
}

#[test]
#[doc = "spec: 1.19:2"]
fn what_is_checked_is_what_runs() {
    // The random program generator agrees three ways on at least 10,000
    // programs under the extended suite, with no more than half a percent
    // inconclusive, and its fragment covers arithmetic in both overflow
    // configurations, panics, mutation, bounded loops, and &mut calls. The
    // run is `tests/random_programs.rs` under LOCUS_EXTENDED; what is
    // pinned here is that it is configured as the criterion says.
    let count: u64 = constant_in("random_programs.rs", "EXTENDED_COUNT")
        .parse()
        .unwrap();
    assert!(count >= 10_000, "{count}");
    let inconclusive: f64 = constant_in("random_programs.rs", "MAX_INCONCLUSIVE")
        .parse()
        .unwrap();
    assert!(inconclusive <= 0.005, "{inconclusive}");
    let text = read(&root().join("tests/random_programs.rs"));
    for covered in [
        "Overflow::ALL",
        "panicked",
        "assigning",
        "looping_assignment",
        "taking_mut",
        "lending",
    ] {
        assert!(
            text.contains(covered),
            "the generator's summary lacks {covered}"
        );
    }
    println!(
        "criterion: what is checked is what runs: tests/random_programs.rs compares {count} \
         programs three ways under LOCUS_EXTENDED, at most {inconclusive} inconclusive, over \
         arithmetic in both builds, panics, mutation, loops, and &mut calls"
    );
}

#[test]
fn the_trusted_base_is_written_down_and_tested_as_such() {
    // Every axiom, proof rule, and primitive the kernel names is named in
    // the Kernel contract; a fabricated constraint is rejected by the
    // kernel as the bad proof it is; the models agree with Rust
    // exhaustively at 8 bits; and no proof is accepted for a false claim.
    // The mutants and the wider types are `tests/kernel_soundness.rs`,
    // `tests/kernel_ops.rs`, `tests/kernel_machine.rs`, and
    // `tests/kernel_lemmas.rs`.
    let term = read(&root().join("src/kernel/term.rs"));
    let mut names = Vec::new();
    for function in [
        "fn name(self) -> &'static str {",
        "fn name(&self) -> &'static str {",
        "fn rule_name(&self) -> &'static str {",
    ] {
        let mut from = 0;
        while let Some(found) = term[from..].find(function) {
            let start = from + found + function.len();
            let end = start + term[start..].find("\n    }\n").expect("the function ends");
            for line in term[start..end].lines() {
                if let Some(rest) = line.split("=> \"").nth(1)
                    && let Some(name) = rest.split('"').next()
                    && !names.contains(&name.to_string())
                {
                    names.push(name.to_string());
                }
            }
            from = end;
        }
    }
    assert!(
        names.len() >= 60,
        "{} names scraped: {names:?}",
        names.len()
    );
    let contract = atlas_document("kernel-contract");
    let missing: Vec<&String> = names
        .iter()
        .filter(|name| !mentions(&contract, name))
        .collect();
    assert!(
        missing.is_empty(),
        "the Kernel contract does not name: {missing:?}"
    );

    let (definitions, _) = Definitions::with_prelude();
    let mut ctx = Context::with_definitions(Rc::new(definitions));
    let lo = Term::var(ctx.declare(Type::machine(MachineInt::U32)).unwrap());
    let goal = Term::int_le(
        Term::view(MachineInt::U32, lo.clone()),
        Term::int(4_294_967_295),
    );
    let fabricated = HypId::fresh();
    let proof = Proof::linear(
        goal.clone(),
        1,
        vec![(Proof::Hyp(HypRef::Free(fabricated)), 1)],
    );
    assert_eq!(
        infer_proof(&mut ctx, &proof),
        Err(KernelError::UnknownHypothesis(fabricated))
    );
    let proof = Proof::linear(
        goal.clone(),
        1,
        vec![(Proof::Axiom(Axiom::ViewUpper(MachineInt::U32, lo)), 1)],
    );
    assert_eq!(check_proof(&mut ctx, &proof, &goal), Ok(()));

    let mut pairs = 0;
    for a in 0..=255u8 {
        for b in 0..=255u8 {
            let sum = Term::op(
                Op::WrappingAdd,
                MachineInt::U8,
                vec![Term::U8(a), Term::U8(b)],
            );
            let Ok(Term::Eq(_, _, value)) = infer_proof(&mut ctx, &Proof::Literal(sum)) else {
                panic!("no literal step for {a} + {b}")
            };
            assert_eq!(*value, Term::U8(a.wrapping_add(b)));
            let below = Term::cmp(CmpOp::Lt, MachineInt::U8, Term::U8(a), Term::U8(b));
            let Ok(Term::Eq(_, _, value)) = infer_proof(&mut ctx, &Proof::Literal(below)) else {
                panic!("no literal step for {a} < {b}")
            };
            assert_eq!(*value, Term::Bool(a < b));
            pairs += 1;
        }
    }
    let falsehood = Term::int_le(Term::int(1), Term::int(0));
    assert!(check_proof(&mut ctx, &Proof::Evaluate(falsehood.clone()), &falsehood).is_err());
    println!(
        "criterion: the trusted base is written down and tested as such: {} kernel names, all \
         in the Kernel contract; a fabricated constraint is rejected; wrapping_add and < at u8 \
         agree with Rust on {pairs} pairs; a false claim is refused",
        names.len()
    );
}

#[test]
#[doc = "spec: 1.0:2"]
fn the_parser_is_robust_and_total_for_stated_reasons() {
    // Totality rests on two guarantees tested on their own: every loop of
    // the parser consumes a token or stops, so the steps stay within four
    // times the tokens, here over every corpus file; and recursion is
    // bounded, so input nested 512 deep reports the limit on a 1 MB stack.
    // The fuzz tests are `tests/parser_fuzz.rs`, at a hundred times their
    // counts under LOCUS_EXTENDED.
    let mut files = 0;
    let mut worst = 0.0f64;
    for directory in [
        "examples",
        "tests/corpus/accept",
        "tests/corpus/reject",
        "tests/corpus/target",
    ] {
        for (name, text) in files_in(directory) {
            let mut sources = SourceMap::default();
            let file = sources.add(&name, &text);
            let parsed = parse(sources.get(file));
            let ratio = parsed.stats.steps as f64 / parsed.stats.tokens as f64;
            assert!(
                parsed.stats.steps <= 4 * parsed.stats.tokens,
                "{name}: {ratio}"
            );
            worst = worst.max(ratio);
            files += 1;
        }
    }
    let deep = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let mut limits = 0;
            for (open, close) in [("(", ")"), ("{", "}"), ("if c { ", " } else { 1 }")] {
                let text = format!(
                    "fn f(c: bool) -> u8 {{ {}1{} }}",
                    open.repeat(512),
                    close.repeat(512)
                );
                let mut sources = SourceMap::default();
                let file = sources.add("deep.lc", &text);
                let parsed = parse(sources.get(file));
                assert!(
                    parsed.diagnostics.iter().any(|d| d.code == "L0108"),
                    "{open}: no limit reported"
                );
                limits += 1;
            }
            limits
        })
        .unwrap()
        .join()
        .unwrap();
    println!(
        "criterion: the parser is robust, and total for stated reasons: steps at most {worst:.2} \
         times the tokens over {files} corpus files; {deep} nesting forms 512 deep report the \
         limit on a 1 MB stack; the fuzz tests run at a hundred times their counts under \
         LOCUS_EXTENDED"
    );
}

#[test]
fn the_legacy_is_gone() {
    // No math fn, no `[P]` or `@[P]`, no state-passing loop or bounded for,
    // no `Nat` in the kernel, no proof by 256 cases: each spelling is an
    // ordinary syntax error, and the source has no path for any of them.
    let spellings = [
        "math fn f() -> u8 { 1 }",
        "def f() -> u8 { 1 }",
        "fn f() -> Prop { [true] }",
        "fn f(n: u8) -> (out: u8, @[out == n]) { (n, _) }",
        "fn f() -> u8 { loop (i: u8 = 0) -> u8 { break i } }",
        "fn f(n: u8) -> u8 { for i in 0..n (s: u8 = 0) { continue(s) } n }",
        "fn f() -> u8 { loop { continue(1) } }",
        "fn f(n: u8) -> (out: u8, @(out == n)) { (n, @{ _ }) }",
        "fn f(n: u8, small: #Small(n)) -> u8 { n }",
    ];
    for text in spellings {
        let mut sources = SourceMap::default();
        let _file = sources.add("legacy.lc", text);
        let (accepted, _) = diagnostics_of("legacy.lc", text);
        assert!(!accepted, "accepted legacy spelling: {text}");
    }
    let (accepted, diagnostics) = diagnostics_of("nat.lc", "fn f(n: Nat) -> u8 { 0 }");
    assert!(!accepted);
    assert_eq!(diagnostics[0].code, "L0201");
    let mut leftovers = Vec::new();
    for path in source_files() {
        let text = read(&path);
        let relative = path
            .strip_prefix(root())
            .unwrap()
            .to_string_lossy()
            .into_owned();
        for word in [
            "StateParameter",
            "at_math_fn",
            "L0113",
            "L0114",
            "L0117",
            "EvaluateAll",
            "evaluate_all",
            "NatInduction",
            "nat_induction",
            "nat_add",
            "to_nat",
            "of_nat",
        ] {
            if mentions(&text, word) {
                leftovers.push(format!("{relative}: {word}"));
            }
        }
        if text.contains("math fn") || text.contains("Type::Nat") || text.contains("Term::Nat") {
            leftovers.push(format!("{relative}: math fn or Nat"));
        }
    }
    assert!(leftovers.is_empty(), "{leftovers:?}");
    println!(
        "criterion: the legacy is gone: {} retired spellings are syntax errors, `Nat` is no \
         type, and the source has no path for math fn, brackets, state-passing loops, Nat, or \
         a proof by 256 cases",
        spellings.len()
    );
}

#[test]
fn the_suites_are_usable() {
    // tools/check.sh is the gate of every commit and has an extended form
    // before a milestone; the time of the fast form is measured by the
    // script itself, which records it and reports a run over two minutes.
    let script = read(&root().join("tools/check.sh"));
    assert!(script.contains("--extended"));
    assert!(script.contains("LOCUS_EXTENDED=1"));
    assert!(script.contains("FAST_LIMIT_SECONDS=120"));
    assert!(script.contains("cargo fmt --check"));
    assert!(script.contains("clippy") && script.contains("-D warnings"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root().join("tools/check.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "tools/check.sh is not executable");
    }
    // Exercise completion, failure and signal interruption. A plausible test
    // tally is not evidence that the selected gate reached its final command.
    let receipt = Command::new("python3")
        .arg(root().join("tools/test_gate.py"))
        .output()
        .unwrap();
    assert!(
        receipt.status.success(),
        "{}",
        String::from_utf8_lossy(&receipt.stderr)
    );
    // The fast tests use fixed seeds: the randomized files name theirs.
    for file in [
        "random_programs.rs",
        "parser_fuzz.rs",
        "kernel_soundness.rs",
        "store.rs",
    ] {
        let text = read(&root().join("tests").join(file));
        assert!(text.contains("SEED: u64 = 0x"), "{file} has no fixed seed");
    }
    println!(
        "criterion: the suites are usable: tools/check.sh emits a final completion receipt; \
         failed or killed runs cannot claim completion; fast timings are recorded and \
         --extended runs the long forms under LOCUS_EXTENDED=1 in release"
    );
}

#[test]
fn target_programs_are_exactly_the_atlas_vision_examples() {
    let source = atlas_document("target-examples");
    let blocks: Vec<_> = source
        .split("~~~")
        .enumerate()
        .filter_map(|(i, s)| (i % 2 == 1).then_some(s.trim()))
        .filter(|s| s.contains("fn "))
        .collect();
    assert_eq!(
        blocks.len(),
        TARGET.len(),
        "each target has one complete program fence"
    );
    for (name, expected) in TARGET.into_iter().zip(blocks) {
        let actual = read(&target(name));
        let actual = actual.split("//~").next().unwrap().trim();
        assert_eq!(
            actual, expected,
            "{name}: target program changed independently of Vision"
        );
    }
}
