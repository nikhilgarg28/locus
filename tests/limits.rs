//! Generated boundary inputs keep resource diagnostics covered without giant fixtures.
use locus::store::text::{Names, parse_term};
use locus::{kernel::Context, lexer::lex, limits::*, parser::parse, source::SourceMap};
use std::{collections::BTreeSet, fs, path::Path, process::Command};

#[test]
#[doc = "spec: 2.40:1"]
fn diagnostic_ceiling_is_an_explicit_error_and_never_success() {
    for count in [MAX_DIAGNOSTICS, MAX_DIAGNOSTICS + 1, MAX_DIAGNOSTICS * 2] {
        let mut sources = SourceMap::default();
        let id = sources.add("generated-errors.lc", "💥".repeat(count));
        let parsed = parse(sources.get(id));
        assert!(!parsed.is_success());
        assert_eq!(parsed.diagnostics.len(), MAX_DIAGNOSTICS);
        assert_eq!(
            parsed.diagnostics.last().unwrap().code == "L0011",
            count > MAX_DIAGNOSTICS
        );
        if count > MAX_DIAGNOSTICS {
            assert!(
                parsed
                    .diagnostics
                    .last()
                    .unwrap()
                    .message
                    .contains("MAX_DIAGNOSTICS")
            );
            assert!(
                parsed
                    .diagnostics
                    .last()
                    .unwrap()
                    .notes
                    .iter()
                    .any(|n| n.contains("omitted"))
            );
        }
    }
}

#[test]
#[doc = "spec: 2.40:1"]
fn oversized_source_is_refused_before_tokenization() {
    let mut sources = SourceMap::default();
    let id = sources.add("large.lc", " ".repeat(MAX_SOURCE_BYTES + 1));
    let result = lex(sources.get(id));
    assert_eq!(result.tokens.len(), 1, "only EOF; no partial program");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, "L0010");
    assert!(
        result.diagnostics[0]
            .message
            .contains(&MAX_SOURCE_BYTES.to_string())
    );
}

#[test]
#[doc = "spec: 2.40:1"]
fn driver_refuses_sparse_oversized_file_without_reading_it() {
    let path = std::env::temp_dir().join(format!("locus-source-limit-{}.lc", std::process::id()));
    fs::File::create(&path)
        .unwrap()
        .set_len(MAX_SOURCE_BYTES as u64 + 1)
        .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(
        stderr.contains("L0010") && stderr.contains("MAX_SOURCE_BYTES"),
        "{stderr}"
    );
}

#[test]
#[doc = "spec: 2.40:2"]
fn stored_integer_limit_counts_digits_without_the_sign() {
    let ctx = Context::new();
    let names = Names::new();
    for sign in ["", "-"] {
        let fits = format!("{sign}{}i", "9".repeat(MAX_PROOF_DIGITS));
        assert!(parse_term(&fits, &ctx, &names).is_ok());
        let too_big = format!("{sign}{}i", "9".repeat(MAX_PROOF_DIGITS + 1));
        let error = parse_term(&too_big, &ctx, &names).unwrap_err();
        assert!(error.to_string().contains("MAX_PROOF_DIGITS"), "{error}");
    }
    let error = parse_term(&" ".repeat(MAX_PROOF_TEXT_BYTES + 1), &ctx, &names).unwrap_err();
    assert!(error.to_string().contains("MAX_PROOF_TEXT_BYTES"));
}

#[test]
#[doc = "spec: 2.40:2"]
fn implementation_limit_registry_matches_the_complete_appendix() {
    let atlas = include_str!("../atlas.html");
    let mut seen = BTreeSet::new();
    for limit in ALL {
        assert!(seen.insert(limit.name), "duplicate limit {}", limit.name);
        let row = format!("| `{}` | {} |", limit.name, limit.value);
        assert_eq!(
            atlas.matches(&row).count(),
            1,
            "missing or duplicated appendix row: {row}"
        );
        assert!(!limit.scope.is_empty() && !limit.failure.is_empty());
    }
    let documented = atlas.matches("| `MAX_").count()
        + atlas.matches("| `DEFAULT_RUN_FUEL` |").count()
        + atlas.matches("| `COUNTEREXAMPLE_BOX` |").count();
    assert_eq!(documented, ALL.len(), "unregistered limit appendix row");
}

fn source_files(path: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().is_some_and(|x| x == "rs") {
            found.push(path);
        }
    }
}

#[test]
#[doc = "spec: 2.40:2"]
fn resource_limits_are_centralized_instead_of_duplicated_constants() {
    let mut files = Vec::new();
    source_files(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
        &mut files,
    );
    for path in files {
        if path.ends_with("src/limits.rs") {
            continue;
        }
        for line in fs::read_to_string(&path).unwrap().lines() {
            let Some((_, tail)) = line.split_once("const ") else {
                continue;
            };
            let name = tail
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap();
            let resource = name.starts_with("MAX_")
                || name.ends_with("_LIMIT")
                || name == "FUEL"
                || name == "STRUCTURAL_DEPTH"
                || name.starts_with("COUNTEREXAMPLE_");
            assert!(
                !resource,
                "{}: centralize limit {name} in src/limits.rs",
                path.display()
            );
        }
    }
}

#[test]
#[doc = "spec: 2.40:2"]
fn normalization_budget_is_reported_only_when_it_is_exhausted() {
    // The real ceiling requires a wide proof with401 independent redexes.
    // Always test the unresolved small control; exercise the expensive exact
    // production boundary in the project's extended suite.
    let extended = std::env::var_os("LOCUS_EXTENDED").is_some();
    let count = if extended {
        MAX_NORMALIZATION_STEPS + 1
    } else {
        4
    };
    let left = (0..count)
        .map(|i| format!("(x, x + {i}).0"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut right = vec!["x"; count];
    right[count - 1] = "x + 1";
    let text = format!(
        "logic fn budget(x: Int) -> @(({left}) == ({})) {{ _ }}",
        right.join(", ")
    );
    let mut sources = SourceMap::default();
    let id = sources.add("normalization-limit.lc", text);
    let source = sources.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let checked = locus::elab::elaborate(source, &parsed.program);
    assert!(!checked.is_success());
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "L0230")
    );
    assert_eq!(
        checked.diagnostics.iter().any(|diagnostic| diagnostic
            .notes
            .iter()
            .any(|note| note.contains("MAX_NORMALIZATION_STEPS"))),
        extended,
        "{:?}",
        checked.diagnostics
    );
}

#[test]
#[doc = "spec: 2.40:2"]
fn bounded_counterexample_search_reports_omitted_enumeration() {
    use locus::{
        arith::{Budget, Counterexample, prove},
        kernel::{Integer, Term, Type},
    };
    let mut ctx = Context::new();
    let vars: Vec<_> = (0..=MAX_COUNTEREXAMPLE_ATOMS)
        .map(|_| Term::var(ctx.declare(Type::Int).unwrap()))
        .collect();
    let sum = vars.into_iter().reduce(Term::int_add).unwrap();
    let goal = Term::int_le(sum, Term::Int(Integer::from(0i64)));
    let budget = Budget {
        eliminations: 0,
        ..Budget::default()
    };
    let failure = prove(&ctx, None, &goal, &budget).unwrap_err();
    assert!(matches!(
        failure.counterexample,
        Counterexample::TooManyAtoms
    ));
    assert!(failure.to_string().contains("MAX_COUNTEREXAMPLE_ATOMS"));
}

#[test]
#[doc = "spec: 2.40:2"]
fn cast_lookahead_exhaustion_is_not_silent_syntax_guessing() {
    let arguments = vec!["u8"; MAX_EXPRESSION_CHAIN].join(", ");
    let mut sources = SourceMap::default();
    let id = sources.add(
        "cast-lookahead.lc",
        format!("fn f() -> u8 {{ 0 as Many<{arguments}> }}"),
    );
    let parsed = parse(sources.get(id));
    assert!(!parsed.is_success());
    assert!(
        parsed.diagnostics.iter().any(
            |d| d.code == "L0108" && d.notes.iter().any(|n| n.contains("MAX_EXPRESSION_CHAIN"))
        ),
        "{:?}",
        parsed.diagnostics
    );
}
