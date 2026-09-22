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
            .contains("Checked 6 function(s); 4 proof(s) found and accepted by the kernel.")
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
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("pub fn attempts_left(attempts: u8, correct: u8) -> u8 {")
    );
    let output = locus()
        .arg("run")
        .arg(example("lock.lc"))
        .args(["no_such_function"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
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
