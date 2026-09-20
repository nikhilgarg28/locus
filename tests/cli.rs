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
        .arg(example("increment.loc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Parsed 2 declaration(s)"));
    assert!(stdout.contains("types and proofs have not been checked"));
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("ast")
        .arg(example("preserve.loc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Hole"));
}

#[test]
fn tokens_have_spans_and_unsupported_commands_fail() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("tokens")
        .arg(example("increment.loc"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("At"));
    assert!(stdout.contains("Underscore"));
    assert!(!stdout.contains("Hash"));
    assert!(stdout.contains("Eof"));
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("check")
        .arg(example("increment.loc"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn missing_files_are_reported_without_panicking() {
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .args(["parse", "does-not-exist.loc"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot read")
    );
}
