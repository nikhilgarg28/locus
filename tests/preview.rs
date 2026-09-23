//! Preview gates are per-check, closed, and removable at acceptance.

use std::path::Path;
use std::process::Command;

use locus::elab::{Options, elaborate, elaborate_with_options};
use locus::parser::parse;
use locus::preview::{Feature, PreviewOptionsError, Previews, Status};
use locus::source::{FileId, SourceMap, Span};

#[path = "common/corpus.rs"]
mod runner;

#[test]
fn the_registry_is_closed_and_each_gate_has_an_owner() {
    let expected = [
        (Feature::LogicalSplit, "logical-split", "LOC-209"),
        (Feature::NamedProps, "named-props", "LOC-206"),
        (Feature::LogicalData, "logical-data", "LOC-218"),
        (Feature::HeapViews, "heap-views", "LOC-228"),
    ];
    assert_eq!(Feature::ALL.len(), expected.len());
    for (feature, name, owner) in expected {
        assert_eq!(Feature::parse(name), Ok(feature));
        assert_eq!(feature.name(), name);
        assert_eq!(feature.task(), owner);
    }
    let error = Feature::parse("logcial-split").unwrap_err();
    assert!(matches!(error, PreviewOptionsError::Unknown(_)));
    assert!(error.to_string().contains("logical-split"));
}

#[test]
#[doc = "spec: 1.22:2"]
fn enabling_and_stabilization_have_distinct_outcomes() {
    let span = Span::new(FileId(0), 0, 5);
    for feature in Feature::ALL {
        let mut previews = Previews::default();
        match feature.status() {
            Status::Preview => {
                let diagnostic = previews
                    .require(feature, "new construct", span)
                    .unwrap_err();
                assert_eq!(diagnostic.code, "L0255");
                assert_eq!(diagnostic.labels[0].span, span);
                assert!(diagnostic.message.contains(feature.name()));
                assert!(
                    diagnostic
                        .notes
                        .iter()
                        .any(|note| note.contains(feature.task()))
                );
                previews.enable(feature.name()).unwrap();
                previews.enable(feature.name()).unwrap(); // repeated flags are harmless
                assert_eq!(previews.iter().count(), 1);
                assert!(previews.require(feature, "new construct", span).is_ok());
            }
            Status::Stabilized => {
                assert!(previews.require(feature, "new construct", span).is_ok());
                assert_eq!(
                    previews.enable(feature.name()),
                    Err(PreviewOptionsError::Stabilized(feature))
                );
            }
        }
    }
    let message = PreviewOptionsError::Stabilized(Feature::LogicalSplit).to_string();
    assert!(message.contains("stabilized"));
    assert!(message.contains("remove"));
}

#[test]
fn previews_do_not_leak_between_elaborations() {
    let mut sources = SourceMap::default();
    let file = sources.add("preview.lc", "logic fn claim() -> Prop { prop!(true) }");
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    if Feature::LogicalSplit.status() == Status::Stabilized {
        assert!(elaborate(source, &parsed.program).is_success());
        return;
    }
    for _ in 0..2 {
        let off = elaborate(source, &parsed.program);
        assert!(off.diagnostics.iter().any(|error| error.code == "L0255"));
        options.previews.enable("logical-split").unwrap();
        let on = elaborate_with_options(source, &parsed.program, &options);
        assert!(!on.diagnostics.iter().any(|error| error.code == "L0255"));
    }
}

#[test]
fn corpus_directives_reject_unknown_stale_and_unused_gates() {
    let errors = runner::directives("//~ preview: imaginary");
    assert!(
        errors[0]
            .1
            .as_ref()
            .unwrap_err()
            .contains("unknown preview")
    );
    for feature in Feature::ALL {
        let text = format!(
            "//~ preview: {}\nfn ordinary() -> u8 {{ 0 }}",
            feature.name()
        );
        let examined = runner::examine("preview.lc", &text);
        assert!(
            !examined.failures.is_empty(),
            "an unused directive must fail"
        );
        let failures = examined
            .failures
            .iter()
            .map(|failure| failure.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        match feature.status() {
            Status::Preview => assert!(
                failures.contains("must be rejected with L0255"),
                "{failures}"
            ),
            Status::Stabilized => assert!(failures.contains("stabilized"), "{failures}"),
        }
    }
}

#[test]
fn cli_validates_preview_options_before_opening_files() {
    for (flags, message) in [
        (vec!["--preview"], "takes a feature name"),
        (vec!["--preview", "--stats"], "takes a feature name"),
        (vec!["--preview", "imaginary"], "unknown preview feature"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_locus"))
            .args(["check", "does-not-exist.lc"])
            .args(flags)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8(output.stderr).unwrap().contains(message));
    }
}

#[test]
fn cli_enables_multiple_previews_for_checked_commands() {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("preview_cli.lc");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "fn identity(n: u8) -> u8 { n }").unwrap();
    let active: Vec<_> = Feature::ALL
        .into_iter()
        .filter(|feature| feature.status() == Status::Preview)
        .collect();
    for command in ["check", "rust"] {
        let mut invocation = Command::new(env!("CARGO_BIN_EXE_locus"));
        invocation
            .arg(command)
            .arg(&path)
            .env("LOCUS_PROOFS", "off");
        for feature in &active {
            invocation.args(["--preview", feature.name()]);
        }
        let output = invocation.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
