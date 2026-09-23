use std::{fs, path::PathBuf, process::Command};
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("library_cli_{name}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}
#[test]
fn explicit_libraries_are_checked_once_and_shared_by_check_run_rust_and_build() {
    let dir = scratch("success");
    let source = dir.join("client.lc");
    let library = dir.join("helpers.lc");
    fs::write(&library, "logic fn twice(n: Int) -> Int { n + n }\n").unwrap();
    fs::write(
        &source,
        "fn run() -> u8 { let proof = prove!(twice(3) == 6); 7 }\n",
    )
    .unwrap();
    for operation in ["check", "rust", "run"] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_locus"));
        cmd.env("LOCUS_PROOFS", "off")
            .arg(operation)
            .arg(&source)
            .arg("--library")
            .arg(&library);
        if operation == "run" {
            cmd.arg("run");
        }
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "{operation}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        if operation == "run" {
            assert_eq!(String::from_utf8_lossy(&out.stdout), "7\n");
        }
    }
    let out = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("build")
        .arg(&source)
        .arg("--library")
        .arg(&library)
        .arg("--out")
        .arg(dir.join("crate_out"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn both_library_and_entry_diagnostics_keep_original_path_and_line() {
    let dir = scratch("diagnostics");
    let source = dir.join("client.lc");
    let library = dir.join("helpers.lc");
    fs::write(
        &library,
        "\nlogic fn wrong() -> @(1 == 2) { prove!(1 == 2) }\n",
    )
    .unwrap();
    fs::write(&source, "fn run() -> u8 { 7 }\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("check")
        .arg(&source)
        .arg("--library")
        .arg(&library)
        .env("LOCUS_PROOFS", "off")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("helpers.lc:2:"), "{err}");
    fs::write(
        &library,
        "logic fn good() -> @(1 == 1) { prove!(1 == 1) }\n\n\n",
    )
    .unwrap();
    fs::write(&source, "\nfn bad() -> u8 { true }\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("check")
        .arg(&source)
        .arg("--library")
        .arg(&library)
        .env("LOCUS_PROOFS", "off")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("client.lc:2:"), "{err}");
}
#[test]
fn incomplete_or_duplicate_libraries_are_not_silently_accepted() {
    let dir = scratch("rejection");
    let source = dir.join("client.lc");
    let library = dir.join("helpers.lc");
    fs::write(&source, "fn run() -> u8 { 7 }\n").unwrap();
    for text in ["fn broken() -> u8 {", "fn run() -> u8 { 8 }"] {
        fs::write(&library, text).unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_locus"))
            .arg("check")
            .arg(&source)
            .arg("--library")
            .arg(&library)
            .env("LOCUS_PROOFS", "off")
            .output()
            .unwrap();
        assert!(!out.status.success(), "{text}");
    }
}
#[test]
fn checked_logical_library_is_directly_usable_from_cli() {
    let dir = scratch("logical");
    let source = dir.join("client.lc");
    fs::write(
        &source,
        "logic fn nonnegative(xs: Seq<Int>) -> @(seq_length(xs) >= 0) { seq_nonnegative(xs) }\n",
    )
    .unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_locus"));
    cmd.arg("check")
        .arg(&source)
        .arg("--library")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("library/logical.lc"))
        .env("LOCUS_PROOFS", "off");
    if locus::preview::Feature::LogicalData.status() == locus::preview::Status::Preview {
        cmd.args(["--preview", "logical-data"]);
    }
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
