//! Litmus examples: `litmus/<topic>/<name>.<outcome>.lc`, where the outcome is
//! `accept`, `reject` or `open`, annotated with `//~` comments. The format is
//! described in `litmus/README.md`.

use std::collections::BTreeMap;

use crate::diagnostics;
use crate::lexer;
use crate::repo::Repo;
use crate::report::Report;
use crate::spec;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Accept,
    Reject,
    Open,
}

impl Outcome {
    pub fn parse(s: &str) -> Option<Outcome> {
        match s {
            "accept" => Some(Outcome::Accept),
            "reject" => Some(Outcome::Reject),
            "open" => Some(Outcome::Open),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Accept => "accept",
            Outcome::Reject => "reject",
            Outcome::Open => "open",
        }
    }

    pub fn is_decided(self) -> bool {
        self != Outcome::Open
    }
}

/// The mode an example is checked in: built by Locus, or exported to Rust.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Build,
    Export,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s {
            "build" => Some(Mode::Build),
            "export" => Some(Mode::Export),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Build => "build",
            Mode::Export => "export",
        }
    }
}

pub struct Example {
    pub path: String,
    pub topic: String,
    pub name: String,
    pub outcome: Outcome,
    pub question: Option<String>,
    /// Cited spec rules, with the line of each citation.
    pub specs: Vec<(usize, String)>,
    pub issue: Option<u32>,
    pub rfc: Option<String>,
    pub mode: Mode,
    pub errors: Vec<ErrorExpectation>,
    pub runs: Vec<RunExpectation>,
    pub source: String,
}

// The litmus runner, which arrives with the compiler, reads every field.
#[allow(dead_code)]
pub struct ErrorExpectation {
    /// The line of the annotation.
    pub line: usize,
    /// The line the error must be reported on.
    pub target: usize,
    pub code: Option<String>,
    pub text: String,
}

pub struct RunExpectation {
    pub line: usize,
    pub call: String,
    pub expected: String,
}

/// Checks every file under `litmus/` and returns the examples that parsed.
pub fn check(repo: &Repo, report: &mut Report) -> Vec<Example> {
    let mut examples = Vec::new();
    let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
    for path in repo.paths.iter().filter(|p| p.starts_with("litmus/")) {
        if path == "litmus/README.md" {
            continue;
        }
        let (topic, name, outcome) = match parse_path(path) {
            Ok(parts) => parts,
            Err(message) => {
                report.error(path, None, message);
                continue;
            }
        };
        let key = (topic.clone(), name.clone());
        if let Some(other) = seen.get(&key) {
            report.error(
                path,
                None,
                format!("the question `{topic}/{name}` already has an example: {other}"),
            );
        } else {
            seen.insert(key, path.clone());
        }
        let text = repo.text(path).unwrap_or_default();
        examples.push(parse(path, &topic, &name, outcome, text, report));
    }
    examples
}

/// Splits `litmus/<topic>/<name>.<outcome>.lc` into its parts.
pub fn parse_path(path: &str) -> Result<(String, String, Outcome), String> {
    const EXPECTED: &str = "expected litmus/<topic>/<name>.<outcome>.lc, with kebab-case names and outcome accept, reject or open";
    let parts: Vec<&str> = path.split('/').collect();
    let [_, topic, file] = parts[..] else {
        return Err(EXPECTED.to_string());
    };
    let stem = file.strip_suffix(".lc").ok_or(EXPECTED)?;
    let (name, outcome) = stem.split_once('.').ok_or(EXPECTED)?;
    let outcome = Outcome::parse(outcome).ok_or(EXPECTED)?;
    if !is_kebab(topic) || !is_kebab(name) {
        return Err(EXPECTED.to_string());
    }
    Ok((topic.to_string(), name.to_string(), outcome))
}

/// Lowercase ASCII words separated by single hyphens.
pub fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

pub fn parse(
    path: &str,
    topic: &str,
    name: &str,
    outcome: Outcome,
    text: &str,
    report: &mut Report,
) -> Example {
    let mut example = Example {
        path: path.to_string(),
        topic: topic.to_string(),
        name: name.to_string(),
        outcome,
        question: None,
        specs: Vec::new(),
        issue: None,
        rfc: None,
        mode: Mode::Build,
        errors: Vec::new(),
        runs: Vec::new(),
        source: text.to_string(),
    };
    let mut mode_given = false;
    let lexed = lexer::lex(text);
    for comment in &lexed.comments {
        let Some(after) = comment.text.strip_prefix("//~") else {
            continue;
        };
        let number = comment.line;
        let trailing = comment.after_code;
        let in_header = comment.in_header;
        let carets = after.chars().take_while(|c| *c == '^').count();
        let directive = after[carets..].trim();
        let (key, value) = match directive.split_once(':') {
            Some((key, value)) => (key.trim(), value.trim()),
            None => (directive, ""),
        };
        let mut fail = |message: &str| report.error(path, Some(number), message);

        if let Some(code) = key.strip_prefix("error") {
            let code = match code {
                "" => None,
                bracketed => match bracketed
                    .strip_prefix('[')
                    .and_then(|c| c.strip_suffix(']'))
                {
                    Some(code) if diagnostics::is_code(code) => Some(code.to_string()),
                    _ => {
                        fail("expected `error[Lnnnn]`, with L and four digits");
                        continue;
                    }
                },
            };
            if code.is_none() && value.is_empty() {
                fail("an error without a code must give its reason: `//~ error: <reason>`");
                continue;
            }
            if !trailing && carets == 0 {
                fail(
                    "an error annotation on its own line must point at the line above with `//~^`",
                );
                continue;
            }
            if carets >= number {
                fail("this `^` points above the first line");
                continue;
            }
            example.errors.push(ErrorExpectation {
                line: number,
                target: number - carets,
                code,
                text: value.to_string(),
            });
            continue;
        }

        if carets > 0 {
            fail("only error annotations can point at an earlier line with `^`");
            continue;
        }
        if trailing {
            fail("only error annotations can follow code on the same line");
            continue;
        }
        match key {
            "run" => match value.split_once(" => ") {
                Some((call, expected))
                    if !call.trim().is_empty() && !expected.trim().is_empty() =>
                {
                    example.runs.push(RunExpectation {
                        line: number,
                        call: call.trim().to_string(),
                        expected: expected.trim().to_string(),
                    });
                }
                _ => fail("expected `//~ run: <call> => <value>` or `=> panic`"),
            },
            "question" | "spec" | "issue" | "rfc" | "mode" if !in_header => {
                fail("file-level annotations belong in the header comment, before any code");
            }
            "question" => {
                if example.question.is_some() {
                    fail("an example asks exactly one question");
                } else if value.is_empty() {
                    fail("the question is empty");
                } else {
                    example.question = Some(value.to_string());
                }
            }
            "spec" => {
                for id in value.split(',').map(str::trim) {
                    if spec::is_valid_id(id) {
                        example.specs.push((number, id.to_string()));
                    } else {
                        fail(&format!("`{id}` is not a valid rule ID"));
                    }
                }
            }
            "issue" => match value.strip_prefix('#').and_then(|n| n.parse::<u32>().ok()) {
                Some(_) if example.issue.is_some() => fail("an example links one issue"),
                Some(n) => example.issue = Some(n),
                None => fail("expected `//~ issue: #<number>`"),
            },
            "rfc" => {
                if value.len() == 4 && value.chars().all(|c| c.is_ascii_digit()) {
                    example.rfc = Some(value.to_string());
                } else {
                    fail("expected `//~ rfc: NNNN`");
                }
            }
            "mode" => match Mode::parse(value) {
                Some(_) if mode_given => fail("an example has one mode"),
                Some(mode) => {
                    example.mode = mode;
                    mode_given = true;
                }
                None => fail("expected `//~ mode: build` or `//~ mode: export`"),
            },
            _ => fail(&format!(
                "unknown annotation `{key}`; expected question, spec, issue, rfc, mode, error or run"
            )),
        }
    }

    if example.question.is_none() {
        report.error(path, None, "missing `//~ question:` in the header");
    }
    match outcome {
        Outcome::Accept | Outcome::Reject if example.specs.is_empty() => report.error(
            path,
            None,
            "a decided example must cite at least one rule with `//~ spec:`",
        ),
        Outcome::Open if example.issue.is_none() => report.error(
            path,
            None,
            "an open example must link its decision-needed issue with `//~ issue: #N`",
        ),
        _ => {}
    }
    match outcome {
        Outcome::Accept if !example.errors.is_empty() => {
            report.error(path, None, "an accepted example cannot expect errors")
        }
        Outcome::Reject if example.errors.is_empty() => report.error(
            path,
            None,
            "a rejected example must expect at least one error with `//~ error`",
        ),
        _ => {}
    }
    if outcome == Outcome::Reject
        && let Some(run) = example.runs.first()
    {
        report.error(
            path,
            Some(run.line),
            "a rejected program never runs; put run expectations in an accepted example",
        );
    }
    example
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_one(path: &str, text: &str) -> (Example, Vec<String>) {
        let (topic, name, outcome) = parse_path(path).unwrap();
        let mut report = Report::default();
        let example = parse(path, &topic, &name, outcome, text, &mut report);
        (example, report.messages())
    }

    #[test]
    fn paths_name_topic_question_and_outcome() {
        let (topic, name, outcome) = parse_path("litmus/overflow/wrapping-add.accept.lc").unwrap();
        assert_eq!(
            (topic.as_str(), name.as_str(), outcome),
            ("overflow", "wrapping-add", Outcome::Accept)
        );
        for bad in [
            "litmus/x.accept.lc",
            "litmus/a/b/c.accept.lc",
            "litmus/Topic/x.accept.lc",
            "litmus/t/x.maybe.lc",
            "litmus/t/x.lc",
            "litmus/t/x.accept.rs",
            "litmus/t/x_y.reject.lc",
        ] {
            assert!(parse_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_a_rejected_example() {
        let text = "\
//~ question: Is overflow ruled out?
//~ spec: arith.overflow.checked, arith.overflow.wrapping
fn inc(x: u8) -> u8 {
    x + 1 //~ error[L0204]: overflow
}
fn f(x: u8) -> u8 {
    x +
//~^ error: the sum may overflow
    1
}
";
        let (example, messages) = parse_one("litmus/arith/inc.reject.lc", text);
        assert!(messages.is_empty(), "{messages:?}");
        assert_eq!(example.question.as_deref(), Some("Is overflow ruled out?"));
        assert_eq!(example.specs.len(), 2);
        assert_eq!(example.errors[0].target, 4);
        assert_eq!(example.errors[0].code.as_deref(), Some("L0204"));
        assert_eq!(example.errors[1].target, 7);
        assert_eq!(example.errors[1].code, None);
    }

    #[test]
    fn parses_the_mode() {
        let header = "//~ question: q\n//~ spec: a.b\n";
        let (example, messages) = parse_one("litmus/t/a.accept.lc", header);
        assert!(messages.is_empty(), "{messages:?}");
        assert_eq!(example.mode, Mode::Build);
        let text = format!("{header}//~ mode: export\n");
        let (example, messages) = parse_one("litmus/t/a.accept.lc", &text);
        assert!(messages.is_empty(), "{messages:?}");
        assert_eq!(example.mode, Mode::Export);
        let text = format!("{header}//~ mode: export\n//~ mode: build\n//~ mode: ship\n");
        let (_, messages) = parse_one("litmus/t/a.accept.lc", &text);
        let path = "litmus/t/a.accept.lc";
        assert_eq!(
            messages,
            [
                format!("error: {path}:4: an example has one mode"),
                format!("error: {path}:5: expected `//~ mode: build` or `//~ mode: export`"),
            ]
        );
    }

    #[test]
    fn reads_annotations_only_from_comments() {
        let text = "\
//~ question: Are annotations read from strings?
//~ spec: a.b
fn f() -> &'static str {
    let _ = \"//~ error: inside a string\";
    /* //~ error: inside a block comment */
    \"x\" //~ run: not here
}
";
        let (example, messages) = parse_one("litmus/t/a.accept.lc", text);
        assert_eq!(
            messages,
            [
                "error: litmus/t/a.accept.lc:6: only error annotations can follow code on the same line"
            ]
        );
        assert!(example.errors.is_empty());
    }

    #[test]
    fn run_expectations_belong_to_accepted_examples() {
        let header = "//~ question: q\n//~ spec: a.b\n";
        let accept = format!("{header}fn f() {{}}\n//~ run: f(1) => 2\n//~ run: f(255) => panic\n");
        let (example, messages) = parse_one("litmus/t/a.accept.lc", &accept);
        assert!(messages.is_empty(), "{messages:?}");
        assert_eq!(example.runs[0].call, "f(1)");
        assert_eq!(example.runs[0].expected, "2");
        assert_eq!(example.runs[1].expected, "panic");
        let reject = format!("{header}fn f() {{}} //~ error: e\n//~ run: f(1) => 2\n");
        let (_, messages) = parse_one("litmus/t/a.reject.lc", &reject);
        assert_eq!(
            messages,
            [
                "error: litmus/t/a.reject.lc:4: a rejected program never runs; put run expectations in an accepted example"
            ]
        );
    }

    #[test]
    fn enforces_outcome_requirements() {
        let (_, messages) = parse_one(
            "litmus/t/a.accept.lc",
            "//~ question: q\nfn f() {} //~ error: no\n",
        );
        assert_eq!(
            messages,
            [
                "error: litmus/t/a.accept.lc: a decided example must cite at least one rule with `//~ spec:`",
                "error: litmus/t/a.accept.lc: an accepted example cannot expect errors",
            ]
        );
        let (_, messages) = parse_one(
            "litmus/t/a.reject.lc",
            "//~ question: q\n//~ spec: a.b\nfn f() {}\n",
        );
        assert_eq!(
            messages,
            [
                "error: litmus/t/a.reject.lc: a rejected example must expect at least one error with `//~ error`"
            ]
        );
        let (_, messages) = parse_one("litmus/t/a.open.lc", "fn f() {}\n");
        assert_eq!(
            messages,
            [
                "error: litmus/t/a.open.lc: missing `//~ question:` in the header",
                "error: litmus/t/a.open.lc: an open example must link its decision-needed issue with `//~ issue: #N`",
            ]
        );
    }

    #[test]
    fn rejects_malformed_annotations() {
        let text = "\
//~ question: q
//~ issue: #3
fn f() {}
//~ spec: a.b
//~ error: alone
//~ error[E0001]: wrong prefix
//~^ run: f() => 1
//~ run: f()
//~ frobnicate: x
fn g() {} //~ run: g() => 1
";
        let (_, messages) = parse_one("litmus/t/a.open.lc", text);
        let path = "litmus/t/a.open.lc";
        assert_eq!(
            messages,
            [
                format!(
                    "error: {path}:4: file-level annotations belong in the header comment, before any code"
                ),
                format!(
                    "error: {path}:5: an error annotation on its own line must point at the line above with `//~^`"
                ),
                format!("error: {path}:6: expected `error[Lnnnn]`, with L and four digits"),
                format!(
                    "error: {path}:7: only error annotations can point at an earlier line with `^`"
                ),
                format!("error: {path}:8: expected `//~ run: <call> => <value>` or `=> panic`"),
                format!(
                    "error: {path}:9: unknown annotation `frobnicate`; expected question, spec, issue, rfc, mode, error or run"
                ),
                format!(
                    "error: {path}:10: only error annotations can follow code on the same line"
                ),
            ]
        );
    }

    #[test]
    fn finds_duplicate_questions_and_stray_files() {
        let repo = Repo::from_files(&[
            ("litmus/README.md", ""),
            ("litmus/t/a.accept.lc", "//~ question: q\n//~ spec: a.b\n"),
            (
                "litmus/t/a.reject.lc",
                "//~ question: q\n//~ spec: a.b\nx //~ error: e\n",
            ),
            ("litmus/t/notes.md", ""),
        ]);
        let mut report = Report::default();
        let examples = check(&repo, &mut report);
        assert_eq!(examples.len(), 2);
        let messages = report.messages();
        assert_eq!(messages.len(), 2, "{messages:?}");
        assert!(messages[0].contains("the question `t/a` already has an example"));
        assert!(messages[1].starts_with("error: litmus/t/notes.md: expected litmus/"));
    }
}
