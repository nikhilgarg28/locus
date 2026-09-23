//! Trust boundaries are visible in deterministic, reviewed output.
use std::{path::Path, process::Command};

#[test]
fn target_examples_and_foreign_contract_have_audit_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, file, heap, logical) in [
        ("lock", "tests/corpus/target/lock.lc", false, false),
        ("midpoint", "tests/corpus/target/midpoint.lc", false, false),
        ("percent", "tests/corpus/target/percent.lc", false, true),
        (
            "trusted_collection",
            "tests/corpus/accept/trusted_collection.lc",
            true,
            false,
        ),
    ] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_locus"));
        cmd.current_dir(root).args(["audit", file]);
        if heap && locus::preview::Feature::HeapViews.status() == locus::preview::Status::Preview {
            cmd.args(["--preview", "heap-views"]);
        }
        if logical
            && locus::preview::Feature::LogicalData.status() == locus::preview::Status::Preview
        {
            cmd.args(["--preview", "logical-data"]);
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stderr.is_empty());
        let actual = String::from_utf8(result.stdout).unwrap();
        let path = root.join("tests/audit").join(format!("{name}.txt"));
        if std::env::var_os("LOCUS_BLESS").is_some_and(|x| x == "1") {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &actual).unwrap();
        }
        assert_eq!(actual, std::fs::read_to_string(&path).unwrap(), "{name}");
        if heap {
            assert!(actual.contains("deliberately false example"));
            assert!(actual.contains("allocation or capacity failure"));
        }
    }
}

#[test]
fn crate_source_inventory_is_sorted_and_ignores_generated_files() {
    let root = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("audit_crate_inventory");
    let _ = std::fs::remove_dir_all(&root);
    for directory in ["", "nested", "target", ".git"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    for path in [
        "b.lc",
        "nested/a.lc",
        "target/generated.lc",
        ".git/hidden.lc",
        "notes.md",
    ] {
        std::fs::write(root.join(path), "fn answer() -> u8 { 7 }").unwrap();
    }
    let paths = locus::audit::source_paths(&[root.clone(), root.join("b.lc")]).unwrap();
    assert_eq!(paths, [root.join("b.lc"), root.join("nested/a.lc")]);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&root, root.join("nested/cycle")).unwrap();
        assert_eq!(
            locus::audit::source_paths(std::slice::from_ref(&root)).unwrap(),
            paths
        );
    }
    assert!(locus::audit::source_paths(&[root.join("notes.md")]).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn crate_audit_checks_every_source_and_reports_its_path() {
    let root = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("audit_crate_cli");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("inner")).unwrap();
    std::fs::write(root.join("first.lc"), "fn first() -> u8 { 1 }").unwrap();
    std::fs::write(
        root.join("inner/second.lc"),
        "fn second() -> u8 { panic!(\"review me\") }",
    )
    .unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_locus"))
            .arg("audit")
            .arg(&root)
            .output()
            .unwrap()
    };
    let checked = run();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let out = String::from_utf8(checked.stdout).unwrap();
    assert!(
        out.contains("first.lc") && out.contains("second.lc") && out.contains("review me"),
        "{out}"
    );
    assert!(out.find("first.lc").unwrap() < out.find("second.lc").unwrap());
    std::fs::write(root.join("inner/second.lc"), "fn second() -> u8 { true }").unwrap();
    assert!(!run().status.success());
    let _ = std::fs::remove_dir_all(root);
}
