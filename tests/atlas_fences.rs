//! Current-language examples are exported from the only documentation copy.
#[path = "common/corpus.rs"]
mod runner;
use locus::{diagnostic::Level, elab::elaborate, parser::parse, source::SourceMap};
use std::process::Command;

fn check_rejection(name: &str, text: &str, expected: &[&str]) -> Result<(), String> {
    let mut sources = SourceMap::default();
    let id = sources.add(name, text);
    let source = sources.get(id);
    let parsed = parse(source);
    let diagnostics = if parsed.is_success() {
        elaborate(source, &parsed.program).diagnostics
    } else {
        parsed.diagnostics
    };
    let actual: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.level == Level::Error)
        .map(|d| d.code)
        .collect();
    if actual.is_empty() || actual != expected {
        return Err(format!(
            "{name}: reject fence expected {expected:?}, got {actual:?}"
        ));
    }
    Ok(())
}

#[test]
fn every_executable_now_fence_checks_and_every_run_agrees_with_rust() {
    let output_dir =
        std::env::temp_dir().join(format!("locus-atlas-fences-{}", std::process::id()));
    std::fs::create_dir_all(&output_dir).unwrap();
    let exported = Command::new("python3")
        .args(["tools/spec.py", "fences", "--out"])
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let manifest = std::fs::read_to_string(output_dir.join("manifest.tsv")).unwrap();
    let mut compiled = Vec::new();
    let mut failures = Vec::new();
    let mut checked = 0;
    for row in manifest.lines() {
        let fields: Vec<_> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{row}");
        let [doc, line, mode, file, errors] = fields[..] else {
            unreachable!()
        };
        let text = std::fs::read_to_string(output_dir.join(file)).unwrap();
        let name = format!("atlas_{doc}_line_{line}");
        match mode {
            "prose" => {}
            "reject" => {
                checked += 1;
                if let Err(why) =
                    check_rejection(&name, &text, &errors.split(',').collect::<Vec<_>>())
                {
                    failures.push(why)
                }
            }
            "check" | "run" => {
                checked += 1;
                let result = runner::examine(&name, &text);
                failures.extend(result.failures.iter().map(ToString::to_string));
                failures.extend(
                    result
                        .inconclusive
                        .iter()
                        .map(|f| format!("inconclusive: {f}")),
                );
                compiled.extend(result.compiled);
            }
            _ => panic!("unknown exported fence mode {mode}"),
        }
    }
    assert!(checked > 0, "Now contains no checked language example");
    let result = runner::compile_and_compare(&compiled, "locus_atlas_fences", runner::TIMEOUT);
    failures.extend(result.failures.iter().map(ToString::to_string));
    failures.extend(
        result
            .inconclusive
            .iter()
            .map(|f| format!("inconclusive: {f}")),
    );
    std::fs::remove_dir_all(output_dir).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn editing_a_fence_to_lie_fails_its_check_run_or_reject_expectation() {
    assert!(
        !runner::examine("check-lie", "fn f()->u8{true}")
            .failures
            .is_empty()
    );
    assert!(
        !runner::examine("run-lie", "fn f()->u8{1}\n//~ run: f() => 2")
            .failures
            .is_empty()
    );
    assert!(check_rejection("unexpected pass", "fn f()->u8{1}", &["L0220"]).is_err());
    assert!(check_rejection("wrong reason", "fn f()->u8{missing}", &["L0220"]).is_err());
    assert!(check_rejection("pinned rejection", "fn f()->u8{true}", &["L0220"]).is_ok());
}
