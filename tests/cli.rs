use std::path::PathBuf;
use std::process::Command;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn parse_reports_syntax_only_and_ast_is_available() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("parse")
        .arg(example("increment.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Parsed 2 declaration(s)"));
    assert!(stdout.contains("types and proofs have not been checked"));
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("ast")
        .arg(example("preserve.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Hole"));
}

#[test]
fn tokens_have_spans_and_unsupported_commands_fail() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("tokens")
        .arg(example("increment.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("At"));
    assert!(stdout.contains("Underscore"));
    assert!(!stdout.contains("Hash"));
    assert!(stdout.contains("Eof"));
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("compile")
        .arg(example("increment.lc"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn missing_files_are_reported_without_panicking() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .args(["parse", "does-not-exist.lc"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot read")
    );
}

#[test]
fn a_file_is_checked_run_and_printed_as_rust() {
    let locus = || Command::new(env!("CARGO_BIN_EXE_locus"));
    let output = locus()
        .arg("check")
        .arg(example("lock.lc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Checked 6 function(s); 5 proof(s) found and accepted by the kernel.")
    );
    let output = locus()
        .arg("check")
        .arg(example("lock.lc"))
        .arg("--holes")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("filled (computed,"), "{stdout}");
    assert!(stdout.contains("filled (evaluation,"), "{stdout}");
    assert!(!stdout.contains("256"), "{stdout}");
    let output = locus()
        .arg("run")
        .arg(example("lock.lc"))
        .args(["attempts_left", "2", "9"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n");
    let output = locus()
        .arg("rust")
        .arg(example("lock.lc"))
        .output()
        .unwrap();
    // Nothing in the example is marked `pub`, so nothing is exported: the
    // functions are printed as written, private.
    let rust = String::from_utf8(output.stdout).unwrap();
    assert!(
        rust.contains("\nfn attempts_left(attempts: u8, correct: u8) -> u8 {"),
        "{rust}"
    );
    assert!(!rust.contains("pub fn"), "{rust}");
    let output = locus()
        .arg("run")
        .arg(example("lock.lc"))
        .args(["no_such_function"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn build_writes_a_crate_that_compiles_and_is_the_same_twice() {
    let locus = || Command::new(env!("CARGO_BIN_EXE_locus"));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli_build_lock");
    let build = || {
        let output = locus()
            .arg("build")
            .arg(example("lock.lc"))
            .arg(example("increment.lc"))
            .arg("--out")
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("Wrote the crate `cli_build_lock`, 2 module(s), under"),
            "{stdout}"
        );
        [
            "Cargo.toml",
            "src/lib.rs",
            "src/increment.rs",
            "src/lock.rs",
        ]
        .map(|path| std::fs::read_to_string(out.join(path)).unwrap())
    };
    let first = build();
    assert!(
        first[1].contains("pub mod increment;\n\npub mod lock;\n"),
        "{}",
        first[1]
    );
    assert!(
        first[3].contains("\nfn attempts_left(attempts: u8, correct: u8) -> u8 {"),
        "{}",
        first[3]
    );
    // The crate compiles as a library under `-D warnings`, with the edition
    // its manifest names.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let output = Command::new(rustc)
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "-D",
            "warnings",
            "-o",
        ])
        .arg(out.join("libcli_build_lock.rlib"))
        .arg(out.join("src/lib.rs"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(build(), first);
    // Usage errors, and a file that is refused, write no crate.
    let output = locus()
        .arg("build")
        .arg(example("lock.lc"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("`--out <dir>` says where the crate goes")
    );
    let refused = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli_build_refused");
    let output = locus()
        .arg("build")
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/corpus/reject/pub_evidence_fn.lc"),
        )
        .arg("--out")
        .arg(&refused)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("error[L0244]")
    );
    assert!(!refused.exists());
    // A file whose name is no module name.
    let odd = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("Odd-Name.lc");
    std::fs::write(&odd, "fn f() -> u8 { 1 }\n").unwrap();
    let output = locus()
        .arg("build")
        .arg(&odd)
        .arg("--out")
        .arg(&refused)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("a module's name is a lower-case identifier")
    );
}

/// The `--holes` listing without its timings, which are the one thing in it
/// that may differ between runs.
fn holes_without_timings(stdout: &str) -> String {
    stdout
        .lines()
        .map(|line| match line.rfind(", ") {
            Some(cut) if line.ends_with(" us)") => format!("{})", &line[..cut]),
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn checking_is_deterministic() {
    // Which tier fills each hole, and how large the proof is, is a function
    // of the file alone: two runs over every example agree byte for byte.
    let examples = std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "lc"));
    let mut seen = 0;
    for path in examples {
        let run = || {
            let output = Command::new(env!("CARGO_BIN_EXE_locus"))
                .arg("check")
                .arg(&path)
                .arg("--holes")
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", path.display());
            holes_without_timings(&String::from_utf8(output.stdout).unwrap())
        };
        let (first, second) = (run(), run());
        assert_eq!(first, second, "{}", path.display());
        assert!(first.contains("filled ("), "{}: {first}", path.display());
        seen += 1;
    }
    assert!(seen >= 5);
}

/// The lines of `--stats` that do not carry a timing: the counts of the
/// obligations by tier, and the list of them.
fn stats_without_timings(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip_while(|line| !line.starts_with("obligations:"))
        .map(str::to_string)
        .collect()
}

#[test]
fn stats_count_the_obligations_by_tier() {
    // The target examples of the atlas predict, over the whole 32-bit lock
    // and midpoint, three exact, four computed, and seven arithmetic
    // obligations. What the tiers count differs, and the difference is
    // recorded rather than adjusted: `0 <= 3` and `2 != 0` on literals are
    // decided by evaluation, which the prediction folds into computed;
    // and an overflow row of the table has two premises, `min <= e` and
    // `e <= max`, where the prediction counts the operator once, so each
    // of the four operators `+` and `-` adds an arithmetic obligation for
    // the bound the ranges of the views give.
    let corpus = |directory: &str, name: &str| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus")
            .join(directory)
            .join(name)
    };
    let stats = |path: PathBuf| {
        let output = Command::new(env!("CARGO_BIN_EXE_locus"))
            .arg("check")
            .arg(&path)
            .arg("--stats")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", path.display());
        stats_without_timings(&String::from_utf8(output.stdout).unwrap())
    };
    assert_eq!(
        stats(corpus("target", "midpoint.lc")),
        [
            "obligations: 6 (1 evaluation, 5 arithmetic)",
            "  12:20 arithmetic, 1 pairs",
            "  12:20 arithmetic, 2 pairs",
            "  12:26 evaluation",
            "  13:18 arithmetic, 2 pairs",
            "  13:18 arithmetic, 8 pairs",
            "  14:11 arithmetic, 18 pairs",
            "Checked 1 function(s); 6 proof(s) found and accepted by the kernel.",
        ]
    );
    assert_eq!(
        stats(corpus("accept", "lock32_step.lc")),
        [
            "obligations: 9 (1 exact, 1 computed, 1 evaluation, 6 arithmetic)",
            "  34:40 evaluation",
            "  38:28 arithmetic, 1 pairs",
            "  39:59 arithmetic, 1 pairs",
            "  39:59 exact",
            "  40:44 arithmetic, 4 pairs",
            "  42:65 computed",
            "  51:18 arithmetic, 1 pairs",
            "  51:18 arithmetic, 1 pairs",
            "  52:12 arithmetic, 4 pairs",
            "Checked 3 function(s); 9 proof(s) found and accepted by the kernel.",
        ]
    );
}
