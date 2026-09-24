use locus::project::{self, Build};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
#[test]
fn filesystem_diagnostic_inventory_is_executable() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/project-errors");
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let code = entry.file_name().to_string_lossy().into_owned();
        let case =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("project_diagnostic_{code}"));
        let _ = fs::remove_dir_all(&case);
        copy(&entry.path(), &case);
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(case.join("case.toml")).unwrap()).unwrap();
        let error = match config["operation"].as_str().unwrap() {
            "check" => project::check(&case, &Default::default())
                .err()
                .expect("reject fixture"),
            "rust" => {
                project::rust(project::check(&case, &Default::default()).unwrap()).unwrap_err()
            }
            "cargo" => Build::new(&case)
                .offline(true)
                .check()
                .err()
                .expect("invalid metadata"),
            "build" => {
                fs::create_dir(case.join("out")).unwrap();
                fs::write(case.join("out/locus.rs"), "handwritten").unwrap();
                Build::new(&case)
                    .offline(true)
                    .out_dir(case.join("out"))
                    .generate()
                    .unwrap_err()
            }
            _ => panic!("unknown operation"),
        };
        assert!(
            error.diagnostics.iter().any(|d| d.code == code),
            "{code}: {error}"
        );
        let json = error
            .diagnostics
            .iter()
            .map(|d| d.render_json(&error.sources) + "\n")
            .collect::<String>()
            .replace(case.to_str().unwrap(), "<fixture>");
        for line in json.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(value.is_array());
        }
        let json_path = entry.path().join("expected.jsonl");
        if std::env::var_os("LOCUS_BLESS_PROJECT").is_some() {
            fs::write(&json_path, &json).unwrap();
        }
        assert_eq!(json, fs::read_to_string(json_path).unwrap(), "{code}");
        // Exercise the driver's source mapping and JSON route as well as the
        // public API. The same real fixture must fail for the same reason.
        let mut cli = std::process::Command::new(env!("CARGO_BIN_EXE_locus"));
        let operation = config["operation"].as_str().unwrap();
        cli.arg(if operation == "cargo" {
            "check"
        } else {
            operation
        })
        .arg(&case)
        .args(["--offline", "--no-store", "--error-format", "json"]);
        if operation == "build" {
            cli.arg("--out-dir").arg(case.join("out"));
        }
        let cli = cli.output().unwrap();
        assert!(!cli.status.success(), "{code}");
        let stderr = String::from_utf8(cli.stderr).unwrap();
        assert!(
            stderr.contains(&format!("\"code\":\"{code}\"")),
            "{code}: {stderr}"
        );
        let rendered = error
            .to_string()
            .replace(case.to_str().unwrap(), "<fixture>");
        let golden = entry.path().join("expected.stderr");
        if std::env::var_os("LOCUS_BLESS_PROJECT").is_some() {
            fs::write(&golden, &rendered).unwrap();
        }
        assert_eq!(rendered, fs::read_to_string(golden).unwrap(), "{code}");
    }
}
