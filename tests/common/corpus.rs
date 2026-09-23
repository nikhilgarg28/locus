//! The corpus runner, shared by `tests/corpus.rs` and `tests/acceptance.rs`:
//! a feature is tested by adding a `.lc` file. A test includes this file
//! with `#[path = "common/corpus.rs"] mod runner;`.
//!
//! Every file in `examples`, `tests/corpus/accept`, and `tests/corpus/reject`
//! says what is expected of it in comments that start with `//~`:
//!
//! ~~~text
//! //~ proofs: 8                          the number of proofs found
//! //~ run: attempts_left(2, 9) => 1      a call and its result
//! //~ run: f(255) => panic               a call that panics
//! //~ run: f(255) => panic: no room      ... with exactly this message
//! //~ run: f(255) => panic | 0           a call whose outcome depends on the
//!                                        build: with overflow checks on, then
//!                                        `|`, then with them off
//! //~ run: bump(&mut 3 -> 4) => ()       a `&mut` argument: the value lent,
//!                                        and after `->` the value the place
//!                                        holds when the call has ended, on
//!                                        a return or at a panic; `| v` after
//!                                        it is the value in a build without
//!                                        overflow checks, when it differs
//! //~ run: read(&7) => 7                 a `&` argument: the value lent
//! //~ rust: fn run(n: u8) -> u8 {    text the generated Rust contains
//! //~ error: L0204                       an error reported on this line
//! //~^ error: L0204 unknown name         ... on the line above; `^^` is two
//!                                        above. Text after the code must
//!                                        appear in the message.
//! //~ warning: L0247 unreachable         a warning reported on this line, as
//!                                        `error` is; the file is accepted
//! //~ preview: logical-split          enable and exercise an unfinished feature
//! //~ spec: 1.5:1                      focused rule citation (validated by spec.py)
//! //~ known: LOC-123 reason             a pinned compiler rejection for an open bug
//! //~ parse-only                         the file is parsed and nothing more
//! ~~~
//!
//! A file with an `error` directive must be rejected, with exactly the
//! errors it lists, each on its line. Every warning reported must be listed
//! the same way, and a file with only warnings is accepted like any other.
//! A file in `tests/corpus/target` is
//! the target syntax: one ahead of the elaborator says `parse-only`, and
//! only the parser's diagnostics are compared with its `error` lines; one
//! the elaborator has caught up with drops the directive and is accepted
//! like any other. Any other
//! file must be accepted: it is parsed, elaborated, and checked, every run
//! line is called in the check-IR interpreter and in the erased-tree
//! interpreter, each in both of its modes, overflow checks on and off, and
//! its Rust is printed.
//! The Rust of all accepted files goes into one source file, each in a `mod`
//! of its own, with a `main` that prints one line for every run line: the
//! value, or `panic: ` and the message of a panic it caught. rustc runs
//! twice, with `-D warnings`: once with overflow checks on and once with
//! them off. Each program is run, and its output is compared with the
//! expectations for its build, which are the ones the interpreters were
//! compared with in the matching mode: a run line with one outcome expects
//! it of every build and every mode; one with two, `=> panic | 0`, expects
//! the first with overflow checks on and the second with them off, which
//! is where `+`, `-`, `*`, and unary minus differ between the builds.
//!
//! A run line has three outcomes and a fourth thing that is not one. It
//! holds, it fails, or it is inconclusive: an interpreter ran out of fuel,
//! or the compiled program went without an answer for `TIMEOUT` and was
//! killed. Inconclusive is never a pass, never a failure, and never evidence
//! that the program diverges; it is counted and printed. A program that was
//! killed is started again from the run line after the one it was in, so the
//! rest of the batch is still compared.
//!
//! Values in a run line are written as the interpreters print them, which is
//! also how Rust's `{:?}` prints them: `7`, `true`, `()`, `(1, Erased)`,
//! `Lock { failures: 0, open: false }`, `Wrong`, `NonZero(7, Erased)`. An
//! argument may name its enum, as in `Event::Wrong`; a result is compared as
//! text. A panic's message is written on one line, with `\n` for a newline
//! and `\\` for a backslash. The operators panic since E6, and `=> panic`
//! is also tested below on erased trees that were given a panic by hand.
//!
//! A `&mut` argument is a place the call writes: each interpreter reports
//! the value the callee left in it, and the compiled harness lends a local
//! and reads it after `catch_unwind`, so that what a write before a panic
//! leaves is compared too. The harness answers such a run line with the
//! values of the `&mut` arguments first, `&mut [4, 7] ` before the outcome,
//! which no other answer begins with.
//!
//! `examine` takes a file's name and text and returns every failure in it,
//! not the first, so the runner is tested on itself below with files whose
//! expectations are wrong.
// Each test that includes this file uses a different part of it.
#![allow(dead_code)]

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use locus::diagnostic::Diagnostic;
use locus::elab::{Options, elaborate_with_options};
use locus::preview::{Feature, Previews};
use std::collections::HashMap;

use locus::erased::{
    self, EType, Interpreter, Markers, Module, Outcome, RunError, Value, Visibilities,
    check_module, print_module_with,
};
use locus::exec::{CheckInterpreter, ExecFnId, Lending, Program};
use locus::parser::parse;
use locus::source::{SourceFile, SourceMap};
use locus::typed::Passing;

/// Steps an interpreter may take on one run line.
pub const FUEL: u64 = 10_000_000;

/// How long the compiled program may go without printing an answer before
/// it is killed and the run line it was in is inconclusive. Generous, since
/// being wrong about this costs a comparison and being slow costs nothing
/// when nothing hangs.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// One expectation that did not hold, or one that could not be decided. Line
/// 0 means the file as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            0 => write!(f, "{}: {}", self.file, self.message),
            line => write!(f, "{}:{line}: {}", self.file, self.message),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expected {
    /// The result, as `Value::debug` prints it.
    Value(String),
    /// A panic, with exactly this message when the run line gives one.
    Panic(Option<String>),
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => f.write_str(value),
            Self::Panic(None) => f.write_str("panic"),
            Self::Panic(Some(message)) => write!(f, "panic: {message}"),
        }
    }
}

/// What a run line did, in an interpreter or in the compiled program, and
/// what its `&mut` arguments held afterwards, as `Value::debug` prints them.
pub type Seen = (Observed, Vec<String>);

/// What a run line did, in an interpreter or in the compiled program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
    /// As `Value::debug` and Rust's `{:?}` print it.
    Value(String),
    /// The message, on one line.
    Panic(String),
    /// Out of fuel, or killed at the timeout, with which. Not an outcome of
    /// the program: it may return, panic, or do neither.
    NoAnswer(String),
    /// It could not be run: an interpreter's error, or a program that died.
    Failed(String),
}

impl fmt::Display for Observed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => f.write_str(value),
            Self::Panic(message) => write!(f, "panic: {message}"),
            Self::NoAnswer(why) => write!(f, "no answer: {why}"),
            Self::Failed(why) => write!(f, "error: {why}"),
        }
    }
}

/// A panic's message as the one line it is printed and expected on.
pub fn one_line(message: &str) -> String {
    message
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

impl Observed {
    pub fn of_interpreter(
        result: Result<(Outcome, Vec<Value>), RunError>,
        module: &Module,
        fuel: u64,
    ) -> Seen {
        match result {
            Ok((outcome, lent)) => {
                let lent = lent.iter().map(|value| value.debug(module)).collect();
                let observed = match outcome {
                    Outcome::Value(value) => Self::Value(value.debug(module)),
                    Outcome::Panic(message) => Self::Panic(one_line(&message)),
                    Outcome::OutOfFuel => Self::NoAnswer(format!("out of fuel after {fuel} steps")),
                };
                (observed, lent)
            }
            Err(error) => (Self::Failed(error.to_string()), Vec::new()),
        }
    }

    /// A line the compiled program printed. No value begins with `panic: `,
    /// and only the answer to a run line with `&mut` arguments begins with
    /// `&mut [`: their values, which hold no `]`, then the outcome.
    pub fn of_line(line: &str) -> Seen {
        let (lent, rest) = match line
            .strip_prefix("&mut [")
            .and_then(|rest| rest.split_once("] "))
        {
            Some((lent, rest)) => (split_values(lent), rest),
            None => (Vec::new(), line),
        };
        let observed = match rest.strip_prefix("panic: ") {
            Some(message) => Self::Panic(message.into()),
            None => Self::Value(rest.into()),
        };
        (observed, lent)
    }
}

pub enum Verdict {
    Holds,
    /// What is wrong, to follow the name of the call.
    Fails(String),
    /// Why nothing was learned, to follow the name of the call.
    Inconclusive(String),
}

/// The values of a list as `{:?}` prints them, `a, b`, split at the commas
/// between them and not at those inside a struct or a tuple.
pub fn split_values(list: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    let mut chars = list.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                values.push(current.trim().to_string());
                current.clear();
                if chars.peek() == Some(&' ') {
                    chars.next();
                }
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        values.push(current.trim().to_string());
    }
    values
}

/// An expectation against what was seen. Outcomes are compared as outcomes:
/// a value with a value, a panic with a panic and then the messages. No
/// answer is compared with nothing, so out of fuel can neither satisfy
/// `=> panic` nor contradict `=> 7`. The values the `&mut` arguments hold
/// afterwards are compared as text, after the outcome.
pub fn judge(expected: &Expected, lent: &[String], seen: &Seen) -> Verdict {
    let (observed, left) = seen;
    let holds = match (expected, observed) {
        (_, Observed::NoAnswer(_)) => return Verdict::Inconclusive(format!("gave {observed}")),
        (Expected::Value(expected), Observed::Value(value)) => expected == value,
        (Expected::Panic(None), Observed::Panic(_)) => true,
        (Expected::Panic(Some(expected)), Observed::Panic(message)) => expected == message,
        _ => false,
    };
    if !holds {
        return Verdict::Fails(format!("is `{observed}`, expected `{expected}`"));
    }
    if matches!(observed, Observed::Failed(_)) || left == lent {
        Verdict::Holds
    } else {
        Verdict::Fails(format!(
            "leaves its `&mut` arguments `[{}]`, expected `[{}]`",
            left.join(", "),
            lent.join(", ")
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Directive {
    Spec(Vec<String>),
    Known {
        task: String,
        reason: String,
    },
    Preview(Feature),
    Proofs(usize),
    /// A call and its outcome; `wrapping` is the outcome in a build
    /// without overflow checks, when the line gives one.
    Run {
        call: String,
        expected: Expected,
        wrapping: Option<Expected>,
    },
    Rust(String),
    /// The line is the one the error is expected on, not the directive's.
    Error {
        line: usize,
        code: String,
        message: String,
    },
    /// As `Error`, for a warning; the file is still accepted.
    Warning {
        line: usize,
        code: String,
        message: String,
    },
    /// The file is only parsed.
    ParseOnly,
}

/// The directives of a file, each with the line it is written on; a comment
/// that starts with `//~` and is not a directive is there with what is wrong.
pub fn directives(text: &str) -> Vec<(usize, Result<Directive, String>)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let Some((_, rest)) = line.split_once("//~") else {
            continue;
        };
        let above = rest.chars().take_while(|&c| c == '^').count();
        let rest = &rest[above..];
        if above == 0 && rest.trim() == "parse-only" {
            found.push((number, Ok(Directive::ParseOnly)));
            continue;
        }
        let Some((key, value)) = rest.split_once(':') else {
            found.push((number, Err("a directive reads `//~ key: value`".into())));
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if above > 0 && key != "error" && key != "warning" {
            found.push((
                number,
                Err(format!("`^` belongs to `error` or `warning`, not `{key}`")),
            ));
            continue;
        }
        let directive = match key {
            "spec" => {
                let ids: Vec<_> = value.split([',', ' ']).filter(|s| !s.is_empty()).collect();
                if ids.is_empty()
                    || ids.iter().any(|id| {
                        let parts: Vec<_> = id.split(['.', ':']).collect();
                        parts.len() != 3
                            || !id.contains('.')
                            || !id.contains(':')
                            || parts
                                .iter()
                                .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
                    })
                {
                    Err("a spec citation reads `chapter.section:paragraph`".into())
                } else {
                    Ok(Directive::Spec(
                        ids.into_iter().map(str::to_owned).collect(),
                    ))
                }
            }
            "known" => match value.split_once(' ') {
                Some((task, reason))
                    if task.strip_prefix("LOC-").is_some_and(|n| {
                        !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
                    }) && !reason.trim().is_empty() =>
                {
                    Ok(Directive::Known {
                        task: task.into(),
                        reason: reason.trim().into(),
                    })
                }
                _ => Err("a known-bug marker needs `LOC-NNN` and a reason".into()),
            },
            "preview" => {
                let mut previews = Previews::default();
                previews
                    .enable(value)
                    .map_err(|error| error.to_string())
                    .and_then(|()| {
                        Feature::parse(value)
                            .map(Directive::Preview)
                            .map_err(|error| error.to_string())
                    })
            }
            "proofs" => value
                .parse()
                .map(Directive::Proofs)
                .map_err(|_| format!("`proofs` takes a number, and `{value}` is not one")),
            "run" => match value.rsplit_once("=>") {
                Some((call, expected)) => {
                    let (checked, wrapping) = match expected.split_once('|') {
                        Some((checked, wrapping)) => (checked, Some(wrapping)),
                        None => (expected, None),
                    };
                    Ok(Directive::Run {
                        call: call.trim().into(),
                        expected: expectation(checked),
                        wrapping: wrapping.map(expectation),
                    })
                }
                None => Err("a run line reads `f(arguments) => value`".into()),
            },
            "rust" => Ok(Directive::Rust(value.into())),
            "error" | "warning" => {
                let (code, message) = value.split_once(' ').unwrap_or((value, ""));
                let is_code = code.len() == 5
                    && code.starts_with('L')
                    && code[1..].chars().all(|c| c.is_ascii_digit());
                if !is_code {
                    Err(format!("`{code}` is not an error code such as `L0204`"))
                } else if above >= number {
                    Err("there is no line that far above".into())
                } else {
                    let (line, code, message) =
                        (number - above, code.into(), message.trim().into());
                    Ok(if key == "error" {
                        Directive::Error {
                            line,
                            code,
                            message,
                        }
                    } else {
                        Directive::Warning {
                            line,
                            code,
                            message,
                        }
                    })
                }
            }
            other => Err(format!(
                "unknown directive `{other}`; there are `proofs`, `run`, `rust`, `error`, `warning`, `preview`, `spec`, `known`, and `parse-only`"
            )),
        };
        found.push((number, directive));
    }
    found
}

/// One outcome of a run line: a value, `panic`, or `panic: message`.
pub fn expectation(text: &str) -> Expected {
    match text.trim() {
        "panic" => Expected::Panic(None),
        other => match other.strip_prefix("panic:") {
            Some(message) => Expected::Panic(Some(message.trim().into())),
            None => Expected::Value(other.into()),
        },
    }
}

/// Whether the file says it must be rejected.
pub fn expects_rejection(text: &str) -> bool {
    directives(text)
        .iter()
        .any(|(_, directive)| matches!(directive, Ok(Directive::Error { .. })))
}

/// Whether the file says it is only parsed.
pub fn is_parse_only(text: &str) -> bool {
    directives(text)
        .iter()
        .any(|(_, directive)| matches!(directive, Ok(Directive::ParseOnly)))
}

/// An accepted file's part of the one program rustc compiles.
pub struct Compiled {
    pub file: String,
    pub module: String,
    pub rust: String,
    pub debug_support: String,
    pub runs: Vec<CompiledRun>,
}

pub struct CompiledRun {
    pub line: usize,
    /// The call as a Rust expression, by the module's path, which resolves
    /// from inside the module once it imports the crate's root; for a run
    /// line with `&mut` arguments, a block that lends locals to the call
    /// under `catch_unwind` and reads them afterwards.
    pub call: String,
    pub expected: Expected,
    /// The outcome in a build without overflow checks, when it differs.
    pub wrapping: Option<Expected>,
    /// What each `&mut` argument holds afterwards, as text: with overflow
    /// checks on, and without when it differs.
    pub lent: Vec<(String, Option<String>)>,
}

/// How the compiled program treats arithmetic overflow. The harness is built
/// once for each, because overflow is where a debug build and a release
/// build of the same Rust differ: a panic in one and wrapping in the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    Checked,
    Wrapping,
}

impl Overflow {
    pub const ALL: [Self; 2] = [Self::Checked, Self::Wrapping];

    pub fn flag(self) -> &'static str {
        match self {
            Self::Checked => "overflow-checks=on",
            Self::Wrapping => "overflow-checks=off",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Checked => "overflow checks on",
            Self::Wrapping => "overflow checks off",
        }
    }

    /// The interpreters' mode that matches the build.
    pub fn mode(self) -> erased::Overflow {
        match self {
            Self::Checked => erased::Overflow::Checks,
            Self::Wrapping => erased::Overflow::Wrap,
        }
    }
}

impl CompiledRun {
    /// What the run line expects of a build: its one outcome, or, for a
    /// build without overflow checks, the outcome after `|` when the line
    /// gives one.
    pub fn expected_in(&self, build: Overflow) -> &Expected {
        match (build, &self.wrapping) {
            (Overflow::Wrapping, Some(wrapping)) => wrapping,
            _ => &self.expected,
        }
    }

    /// What the run line expects each `&mut` argument to hold afterwards
    /// in a build.
    pub fn lent_in(&self, build: Overflow) -> Vec<String> {
        self.lent
            .iter()
            .map(|(checked, wrapping)| match (build, wrapping) {
                (Overflow::Wrapping, Some(wrapping)) => wrapping.clone(),
                _ => checked.clone(),
            })
            .collect()
    }

    /// Whether the run line has `&mut` arguments, and the answer is the
    /// values they hold followed by the outcome.
    pub fn lends(&self) -> bool {
        !self.lent.is_empty()
    }
}

/// What came of comparing: what is wrong, and what could not be decided.
#[derive(Default)]
pub struct Report {
    pub failures: Vec<Failure>,
    pub inconclusive: Vec<Failure>,
}

pub struct Examined {
    pub failures: Vec<Failure>,
    /// Run lines an interpreter ran out of fuel on. Not failures.
    pub inconclusive: Vec<Failure>,
    /// Present when the file was accepted.
    pub compiled: Option<Compiled>,
}

/// What the directives of an accepted file are checked against.
pub struct Subject<'a> {
    pub module: &'a Module,
    /// The check IR the erased tree is compared with. An erased tree that
    /// was built by hand has none, and runs in its own interpreter only.
    pub program: Option<&'a Program>,
    /// What its interpreter needs of the functions with `&mut` parameters.
    pub lending: Option<&'a HashMap<ExecFnId, Lending>>,
    pub proofs: usize,
    /// What the items are marked with, as the file wrote it: private by
    /// default, which is why the harness answers from inside each module.
    pub visibilities: Visibilities,
}

/// Everything wrong with one file, short of compiling its Rust. A panic
/// anywhere in the pipeline is one more failure, so the other files still
/// get their turn.
pub fn examine(name: &str, text: &str) -> Examined {
    catch_unwind(AssertUnwindSafe(|| examine_inner(name, text))).unwrap_or_else(|_| Examined {
        failures: vec![Failure {
            file: name.into(),
            line: 0,
            message: "the pipeline panicked on this file".into(),
        }],
        inconclusive: Vec::new(),
        compiled: None,
    })
}

/// The directives of a text, and what is wrong with those that are not.
pub fn directives_of(name: &str, text: &str) -> (Vec<(usize, Directive)>, Vec<Failure>) {
    let mut found = Vec::new();
    let mut failures = Vec::new();
    for (line, directive) in directives(text) {
        match directive {
            Ok(directive) => found.push((line, directive)),
            Err(message) => failures.push(Failure {
                file: name.into(),
                line,
                message,
            }),
        }
    }
    (found, failures)
}

pub fn examine_inner(name: &str, text: &str) -> Examined {
    let (found, mut failures) = directives_of(name, text);
    let mut fail = |line: usize, message: String| {
        failures.push(Failure {
            file: name.into(),
            line,
            message,
        });
    };

    let mut sources = SourceMap::default();
    let file = sources.add(name, text);
    let source = sources.get(file);
    let parsed = parse(source);
    let parse_only = is_parse_only(text);
    let mut options = Options::default();
    for (_, directive) in &found {
        if let Directive::Preview(feature) = directive {
            options
                .previews
                .enable(feature.name())
                .expect("validated directive");
        }
    }
    if parsed.is_success() && !parse_only {
        // Every declared gate must be necessary, independently of the others.
        // This catches stale directives as a feature is stabilized or a fixture changes.
        for omitted in options.previews.iter() {
            let mut without = Options::default();
            for feature in options
                .previews
                .iter()
                .filter(|feature| *feature != omitted)
            {
                without
                    .previews
                    .enable(feature.name())
                    .expect("validated directive");
            }
            let unenabled = elaborate_with_options(source, &parsed.program, &without);
            if !unenabled.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "L0255"
                    && diagnostic
                        .message
                        .contains(&format!("`{}`", omitted.name()))
            }) {
                fail(
                    0,
                    format!(
                        "preview `{}` must be rejected with L0255 when its flag is absent",
                        omitted.name()
                    ),
                );
            }
        }
    }
    let elaborated = (parsed.is_success() && !parse_only)
        .then(|| elaborate_with_options(source, &parsed.program, &options));
    let diagnostics = match &elaborated {
        Some(elaborated) => &elaborated.diagnostics,
        None => &parsed.diagnostics,
    };

    let reported: Vec<(usize, &Diagnostic)> = diagnostics
        .iter()
        .map(|diagnostic| (line_of(source, diagnostic), diagnostic))
        .collect();
    let rejection = expects_rejection(text);
    let known: Vec<_> = found
        .iter()
        .filter_map(|(at, directive)| match directive {
            Directive::Known { task, reason } => Some((*at, task, reason)),
            _ => None,
        })
        .collect();
    if known.len() > 1 {
        fail(
            known[1].0,
            "one known-bug marker is allowed per case".into(),
        );
    }
    for (at, task, reason) in known {
        if !rejection || parse_only {
            fail(
                at,
                format!(
                    "{task}: a known compiler bug needs pinned error directives and full checking"
                ),
            );
        }
        if !diagnostics
            .iter()
            .any(|d| d.level == locus::diagnostic::Level::Error)
        {
            fail(
                at,
                format!(
                    "{task} unexpectedly passed; remove the known-bug marker ({reason}) and its error expectations"
                ),
            );
        }
    }
    for (line, why) in compare_errors(&found, reported) {
        fail(line, why);
    }
    for (at, directive) in &found {
        if rejection
            && !matches!(
                directive,
                Directive::Error { .. }
                    | Directive::Warning { .. }
                    | Directive::ParseOnly
                    | Directive::Preview(_)
                    | Directive::Spec(_)
                    | Directive::Known { .. }
            )
        {
            fail(
                *at,
                "a file with an `error` directive is not run, so this expects nothing".into(),
            );
        } else if parse_only
            && !matches!(
                directive,
                Directive::Error { .. } | Directive::ParseOnly | Directive::Spec(_)
            )
        {
            fail(
                *at,
                "a parse-only file is only parsed, so this expects nothing".into(),
            );
        }
    }
    if parse_only {
        return Examined {
            failures,
            inconclusive: Vec::new(),
            compiled: None,
        };
    }
    let Some(elaborated) = elaborated.filter(|elaborated| elaborated.is_success() && !rejection)
    else {
        return Examined {
            failures,
            inconclusive: Vec::new(),
            compiled: None,
        };
    };
    let subject = Subject {
        module: elaborated.session.erased(),
        program: Some(elaborated.session.program()),
        lending: Some(elaborated.session.lending()),
        proofs: elaborated.holes.len(),
        visibilities: elaborated.visibilities.clone(),
    };
    let mut examined = examine_accepted(name, found, &subject, FUEL);
    failures.append(&mut examined.failures);
    examined.failures = failures;
    examined
}

/// An erased tree that did not come from its text, with the text's
/// directives: the way to a run line that panics while no source does.
pub fn examine_tree(name: &str, text: &str, module: &Module, fuel: u64) -> Examined {
    let (found, mut failures) = directives_of(name, text);
    let subject = Subject {
        module,
        program: None,
        lending: None,
        proofs: 0,
        visibilities: Visibilities::everything_public(),
    };
    let mut examined = examine_accepted(name, found, &subject, fuel);
    failures.append(&mut examined.failures);
    examined.failures = failures;
    examined
}

/// The directives of an accepted file against what it became: the erased
/// tree is type checked and printed, and every run line is called in each
/// interpreter there is.
pub fn examine_accepted(
    name: &str,
    found: Vec<(usize, Directive)>,
    subject: &Subject,
    fuel: u64,
) -> Examined {
    let module = subject.module;
    let mut failures = Vec::new();
    let mut inconclusive = Vec::new();
    let remark = |line: usize, message: String| Failure {
        file: name.into(),
        line,
        message,
    };
    let mut fail = |line: usize, message: String| failures.push(remark(line, message));
    if let Err(error) = check_module(module) {
        fail(0, format!("the erased tree is not well typed: {error:?}"));
    }
    let rust = print_module_with(module, &subject.visibilities, Markers::Here);
    let mut compiled = Compiled {
        file: name.into(),
        module: module_name(name),
        rust,
        debug_support: debug_support(module),
        runs: Vec::new(),
    };
    for (at, directive) in found {
        match directive {
            Directive::Proofs(expected) => {
                let proofs = subject.proofs;
                if proofs != expected {
                    fail(
                        at,
                        format!("{proofs} proof(s) were found, expected {expected}"),
                    );
                }
            }
            Directive::Rust(expected) => {
                if !compiled.rust.contains(&expected) {
                    fail(
                        at,
                        format!("the generated Rust does not contain `{expected}`"),
                    );
                }
            }
            Directive::Run {
                call,
                expected,
                wrapping,
            } => {
                let (function, arguments, lent) = match parse_call(&call, module) {
                    Ok(parsed) => parsed,
                    Err(why) => {
                        fail(at, format!("`{call}`: {why}"));
                        continue;
                    }
                };
                // A function of the erased tree was accepted, so it has a reference.
                let reference = module.fns[function].reference;
                let run = CompiledRun {
                    line: at,
                    call: String::new(),
                    expected,
                    wrapping,
                    lent: lent
                        .iter()
                        .map(|(after, wrapping)| {
                            (
                                after.debug(module),
                                wrapping.as_ref().map(|value| value.debug(module)),
                            )
                        })
                        .collect(),
                };
                // Each interpreter in each mode, against the expectation
                // of the build the mode matches.
                for build in Overflow::ALL {
                    let results = [
                        subject.program.map(|program| {
                            let mut interpreter =
                                CheckInterpreter::new(program, fuel).with_overflow(build.mode());
                            if let Some(lending) = subject.lending {
                                interpreter = interpreter.with_lending(lending);
                            }
                            (
                                "check-IR interpreter",
                                interpreter.call_lending(reference, arguments.clone()),
                            )
                        }),
                        Some((
                            "erased-tree interpreter",
                            Interpreter::new(module, fuel)
                                .with_overflow(build.mode())
                                .call_lending(reference, arguments.clone()),
                        )),
                    ];
                    for (interpreter, result) in results.into_iter().flatten() {
                        let seen = Observed::of_interpreter(result, module, fuel);
                        let where_ = format!("{interpreter}, {}: `{call}`", build.name());
                        match judge(run.expected_in(build), &run.lent_in(build), &seen) {
                            Verdict::Holds => {}
                            Verdict::Fails(why) => fail(at, format!("{where_} {why}")),
                            Verdict::Inconclusive(why) => {
                                inconclusive.push(remark(at, format!("{where_} {why}")));
                            }
                        }
                    }
                }
                let function = &module.fns[function];
                let arguments: Vec<String> = arguments
                    .iter()
                    .zip(&function.params)
                    .map(|(value, (_, _, ty))| {
                        rust_value_at_type(value, ty, module, &compiled.module)
                    })
                    .collect();
                compiled.runs.push(CompiledRun {
                    call: compiled_call(&compiled.module, function, &arguments),
                    ..run
                });
            }
            Directive::Error { .. }
            | Directive::Warning { .. }
            | Directive::ParseOnly
            | Directive::Preview(_)
            | Directive::Spec(_)
            | Directive::Known { .. } => {}
        }
    }
    Examined {
        failures,
        inconclusive,
        compiled: Some(compiled),
    }
}

/// Every expected error is reported on its line, and nothing else is: what
/// is missing, then what is unexpected, each with its line.
pub fn compare_errors(
    found: &[(usize, Directive)],
    mut reported: Vec<(usize, &Diagnostic)>,
) -> Vec<(usize, String)> {
    let mut failures = Vec::new();
    for (_, directive) in found {
        let (line, code, message, is_error) = match directive {
            Directive::Error {
                line,
                code,
                message,
            } => (line, code, message, true),
            Directive::Warning {
                line,
                code,
                message,
            } => (line, code, message, false),
            _ => continue,
        };
        let kind = if is_error { "error" } else { "warning" };
        let position = reported.iter().position(|(on, diagnostic)| {
            on == line && diagnostic.code == code && diagnostic.is_error() == is_error
        });
        match position {
            Some(position) => {
                let (_, diagnostic) = reported.remove(position);
                if !diagnostic.message.contains(message.as_str()) {
                    failures.push((
                        *line,
                        format!(
                            "{code} says `{}`, which does not contain `{message}`",
                            diagnostic.message
                        ),
                    ));
                }
            }
            None => failures.push((
                *line,
                format!("expected {kind} {code} on this line, and it was not reported"),
            )),
        }
    }
    for (line, diagnostic) in reported {
        let kind = if diagnostic.is_error() {
            "error"
        } else {
            "warning"
        };
        failures.push((
            line,
            format!(
                "unexpected {kind} {}: {}",
                diagnostic.code, diagnostic.message
            ),
        ));
    }
    failures
}

/// The line an error is reported at: where its primary label starts.
pub fn line_of(source: &SourceFile, diagnostic: &Diagnostic) -> usize {
    diagnostic
        .labels
        .iter()
        .find(|label| label.primary)
        .and_then(|label| source.line_column(label.span.start))
        .map_or(0, |(line, _)| line)
}

/// A Rust module name for a file: `examples/lock.lc` is `examples_lock`, and
/// `tests/corpus/accept/values.lc` is `accept_values`.
pub fn module_name(file: &str) -> String {
    let file = file.strip_prefix("tests/corpus/").unwrap_or(file);
    file.strip_suffix(".lc")
        .unwrap_or(file)
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}

/// What is left of a run line's arguments.
pub struct Cursor<'a> {
    pub rest: &'a str,
}

impl<'a> Cursor<'a> {
    pub fn eat(&mut self, token: &str) -> bool {
        self.rest = self.rest.trim_start();
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    pub fn expect(&mut self, token: &str) -> Result<(), String> {
        if self.eat(token) {
            Ok(())
        } else if self.rest.is_empty() {
            Err(format!("expected `{token}` at the end"))
        } else {
            Err(format!("expected `{token}` at `{}`", self.rest))
        }
    }

    /// A name or a number; empty when neither is next.
    pub fn word(&mut self) -> &'a str {
        self.rest = self.rest.trim_start();
        let end = self
            .rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(self.rest.len());
        let (word, rest) = self.rest.split_at(end);
        self.rest = rest;
        word
    }

    /// Values of the given types, separated by commas, up to `close`.
    pub fn values(
        &mut self,
        tys: &[EType],
        module: &Module,
        close: &str,
    ) -> Result<Vec<Value>, String> {
        let mut values = Vec::new();
        for (index, ty) in tys.iter().enumerate() {
            if index > 0 {
                self.expect(",")?;
            }
            values.push(self.value(ty, module)?);
        }
        if self.eat(",") && !self.rest.trim_start().starts_with(close) {
            return Err(format!("more than {} value(s) before `{close}`", tys.len()));
        }
        self.expect(close)?;
        Ok(values)
    }

    /// `{ field: value, ... }` in the declared order, as `{:?}` prints it.
    pub fn fields(
        &mut self,
        fields: &[(&str, &EType)],
        module: &Module,
    ) -> Result<Vec<Value>, String> {
        self.expect("{")?;
        let mut values = Vec::new();
        for (index, (field, ty)) in fields.iter().enumerate() {
            if index > 0 {
                self.expect(",")?;
            }
            let word = self.word();
            if word != *field {
                return Err(format!("expected field `{field}`, found `{word}`"));
            }
            self.expect(":")?;
            values.push(self.value(ty, module)?);
        }
        self.eat(",");
        self.expect("}")?;
        Ok(values)
    }

    /// A value of a known type, so that `Wrong` needs no `Event::`.
    pub fn value(&mut self, ty: &EType, module: &Module) -> Result<Value, String> {
        match ty {
            EType::Boxed(element) => Ok(Value::Tuple(vec![self.value(element, module)?])),
            EType::Buffer(element) | EType::Array(element, _) | EType::Slice(element) => {
                self.expect("[")?;
                let mut values = Vec::new();
                if !self.eat("]") {
                    loop {
                        values.push(self.value(element, module)?);
                        if self.eat("]") {
                            break;
                        }
                        self.expect(",")?;
                        if self.eat("]") {
                            break;
                        }
                    }
                }
                Ok(Value::Buffer(values))
            }
            EType::Ref(_, inner) => {
                self.eat("&");
                self.value(inner, module)
            }
            EType::Bool => match self.word() {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                other => Err(format!("expected a `bool`, found `{other}`")),
            },
            EType::Int(ty) => {
                // A number, possibly negative, within the type's range, as
                // the harness prints one: without a suffix.
                let negative = self.eat("-");
                let word = self.word();
                let value = word
                    .parse::<i128>()
                    .ok()
                    .map(|value| if negative { -value } else { value })
                    .filter(|value| ty.contains(&locus::kernel::Integer::from(*value)));
                match value {
                    Some(value) => Ok(Value::Int(*ty, value)),
                    None => Err(format!(
                        "expected a `{}`, found `{}{word}`",
                        ty.name(),
                        if negative { "-" } else { "" }
                    )),
                }
            }
            EType::Proved => match self.word() {
                "Erased" => Ok(Value::Proved),
                other => Err(format!("expected `Erased`, found `{other}`")),
            },
            EType::Ghost => match self.word() {
                "Erased" => Ok(Value::Ghost),
                other => Err(format!("expected `Erased`, found `{other}`")),
            },
            EType::Tuple(tys) => {
                self.expect("(")?;
                Ok(Value::Tuple(self.values(tys, module, ")")?))
            }
            EType::Struct(id) | EType::StructApplied(id, _) => {
                let item = module
                    .structs
                    .iter()
                    .find(|item| item.id == *id)
                    .ok_or("a struct that was not emitted")?;
                let word = self.word();
                if word != item.name {
                    return Err(format!("expected a `{}`, found `{word}`", item.name));
                }
                let fields: Vec<(&str, &EType)> = item
                    .fields
                    .iter()
                    .map(|(field, ty)| (field.as_str(), ty))
                    .collect();
                Ok(Value::Struct(*id, self.fields(&fields, module)?))
            }
            EType::Enum(id) | EType::EnumApplied(id, _) => {
                let item = module
                    .enums
                    .iter()
                    .find(|item| item.id == *id)
                    .ok_or("an enum that was not emitted")?;
                let mut word = self.word();
                if self.eat("::") {
                    if word != item.name {
                        return Err(format!("expected a `{}`, found `{word}`", item.name));
                    }
                    word = self.word();
                }
                let Some(index) = item.variants.iter().position(|v| v.name == word) else {
                    return Err(format!("`{}` has no variant `{word}`", item.name));
                };
                let variant = &item.variants[index];
                let values = if let Some(fields) = &variant.fields {
                    let fields: Vec<(&str, &EType)> = fields
                        .iter()
                        .map(String::as_str)
                        .zip(&variant.payload)
                        .collect();
                    self.fields(&fields, module)?
                } else if variant.payload.is_empty() {
                    Vec::new()
                } else {
                    self.expect("(")?;
                    self.values(&variant.payload, module, ")")?
                };
                Ok(Value::Variant(*id, index, values))
            }
            EType::Fn(..) => Err("a function cannot be written in a run line".into()),
        }
    }
}

/// `f(arguments)` as the function's index in the erased tree, the values,
/// and for each `&mut` argument what it is expected to hold afterwards:
/// with overflow checks on, and without when the line gives a second
/// value.
pub type ParsedCall = (usize, Vec<Value>, Vec<(Value, Option<Value>)>);

pub fn parse_call(call: &str, module: &Module) -> Result<ParsedCall, String> {
    let Some((name, arguments)) = call.split_once('(') else {
        return Err("a call reads `f(arguments)`".into());
    };
    let name = name.trim();
    let Some(index) = module.fns.iter().position(|f| f.name == name) else {
        return Err(format!(
            "there is no `{name}` in the erased tree; a function that exists only in proofs cannot be run"
        ));
    };
    let function = &module.fns[index];
    let mut cursor = Cursor { rest: arguments };
    let mut values = Vec::new();
    let mut lent = Vec::new();
    for (position, (_, _, ty)) in function.params.iter().enumerate() {
        if position > 0 {
            cursor.expect(",")?;
        }
        match function.passing_of(position) {
            Passing::Value | Passing::MutValue => values.push(cursor.value(ty, module)?),
            Passing::Ref => {
                cursor.expect("&")?;
                values.push(cursor.value(ty, module)?);
            }
            Passing::RefMut => {
                cursor.expect("&mut")?;
                values.push(cursor.value(ty, module)?);
                if !cursor.eat("->") {
                    return Err(
                        "a `&mut` argument reads `&mut value -> value after the call`".into(),
                    );
                }
                let after = cursor.value(ty, module)?;
                let wrapping = if cursor.eat("|") {
                    Some(cursor.value(ty, module)?)
                } else {
                    None
                };
                lent.push((after, wrapping));
            }
        }
    }
    if cursor.eat(",") && !cursor.rest.trim_start().starts_with(')') {
        return Err(format!(
            "more than {} value(s) before `)`",
            function.params.len()
        ));
    }
    cursor.expect(")")?;
    if !cursor.rest.trim().is_empty() {
        return Err(format!(
            "unexpected `{}` after the call",
            cursor.rest.trim()
        ));
    }
    Ok((index, values, lent))
}

/// The call of a run line as the harness makes it: `m::f(a, b)`, or, when
/// the function takes `&mut` or `&` arguments, a block that lends locals
/// to the call under `catch_unwind` and yields the result with what the
/// `&mut` locals hold afterwards, as text, for `answer_lending`.
pub fn compiled_call(module: &str, function: &erased::EFn, arguments: &[String]) -> String {
    if function.lent().is_empty() {
        let arguments: Vec<String> = arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| match function.passing_of(index) {
                Passing::Ref => format!("&({argument})"),
                _ => argument.clone(),
            })
            .collect();
        return format!("{module}::{}({})", function.name, arguments.join(", "));
    }
    let mut lets = Vec::new();
    let mut passed = Vec::new();
    let mut read = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        match function.passing_of(index) {
            Passing::Value | Passing::MutValue => passed.push(argument.clone()),
            Passing::Ref => passed.push(format!("&({argument})")),
            Passing::RefMut => {
                lets.push(format!("let mut a{index} = {argument};"));
                passed.push(format!("&mut a{index}"));
                read.push(format!("format!(\"{{:?}}\", a{index})"));
            }
        }
    }
    format!(
        "{{ {} let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {module}::{}({}))); (result, vec![{}]) }}",
        lets.join(" "),
        function.name,
        passed.join(", "),
        read.join(", ")
    )
}

/// A value as a Rust expression, written from outside the file's module.
pub fn rust_value(value: &Value, module: &Module, path: &str) -> String {
    let all = |values: &[Value]| -> Vec<String> {
        values
            .iter()
            .map(|value| rust_value(value, module, path))
            .collect()
    };
    match value {
        // A bare number: the parameter's type fixes it, a negative one
        // included.
        Value::Buffer(items) => format!("vec![{}]", all(items).join(", ")),
        Value::Bool(_) | Value::Int(..) => value.debug(module),
        Value::Proved | Value::Ghost => format!("{path}::{}", value.debug(module)),
        Value::Tuple(fields) => match all(fields).as_slice() {
            [only] => format!("({only},)"),
            fields => format!("({})", fields.join(", ")),
        },
        Value::Struct(id, fields) => {
            let item = module.structs.iter().find(|item| item.id == *id);
            let item = item.expect("the value was parsed against this module");
            let fields: Vec<String> = item
                .fields
                .iter()
                .zip(all(fields))
                .map(|((name, _), value)| format!("{name}: {value}"))
                .collect();
            format!("{path}::{} {{ {} }}", item.name, fields.join(", "))
        }
        Value::Variant(id, index, payload) => {
            let item = module.enums.iter().find(|item| item.id == *id);
            let item = item.expect("the value was parsed against this module");
            let variant = &item.variants[*index];
            let name = format!("{path}::{}::{}", item.name, variant.name);
            match &variant.fields {
                Some(fields) => {
                    let fields: Vec<String> = fields
                        .iter()
                        .zip(all(payload))
                        .map(|(field, value)| format!("{field}: {value}"))
                        .collect();
                    format!("{name} {{ {} }}", fields.join(", "))
                }
                None if payload.is_empty() => name,
                None => format!("{name}({})", all(payload).join(", ")),
            }
        }
    }
}

/// What the harness answers a run line with: its value as `{:?}` prints it,
/// or `panic: ` and the message of the panic it caught, on one line as
/// `one_line` writes it. Each answer is flushed, so that what was answered
/// before the program is killed is not lost, and the program starts from the
/// run line it is given, so that it can be started again past the one it was
/// killed in. A run line with `&mut` arguments is answered by
/// `answer_lending`, with what they hold afterwards before the outcome
/// (`compiled_call`). `tests/common/compiled.rs` has `answer` alone: the
/// programs it runs lend nothing at the top.
pub const ANSWER: &str = r#"
pub fn render<T: std::fmt::Debug>(result: std::thread::Result<T>) -> String {
    match result {
        Ok(value) => format!("{value:?}"),
        Err(payload) => {
            let message = match (payload.downcast_ref::<&str>(), payload.downcast_ref::<String>()) {
                (Some(message), _) => message.to_string(),
                (_, Some(message)) => message.clone(),
                _ => "a panic with no message".to_string(),
            };
            let message = message.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r");
            format!("panic: {message}")
        }
    }
}

pub fn emit(line: String) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").and_then(|()| out.flush()).expect("stdout is open");
}

pub fn answer<T: std::fmt::Debug>(index: usize, from: usize, call: fn() -> T) {
    if index < from {
        return;
    }
    emit(render(std::panic::catch_unwind(call)));
}

#[allow(dead_code)]
pub fn answer_lending<T: std::fmt::Debug>(
    index: usize,
    from: usize,
    run: impl FnOnce() -> (std::thread::Result<T>, Vec<String>),
) {
    if index < from {
        return;
    }
    let (result, lent) = run();
    emit(format!("&mut [{}] {}", lent.join(", "), render(result)));
}

pub fn main() {
    // A panic is caught and answered with; nothing about it goes to stderr.
    std::panic::set_hook(Box::new(|_| {}));
    let from = std::env::args().nth(1).map_or(0, |from| from.parse().expect("an index"));
"#;

/// Observation-only trait implementations belong to the test harness, so
/// target source need not add Debug merely to be compared with interpreters.
fn debug_support(module: &Module) -> String {
    use locus::typed::Derive;
    fn lifetimes(types: impl Iterator<Item = EType>) -> String {
        fn collect(t: &EType, names: &mut Vec<String>) {
            match t {
                EType::Ref(Some(l), inner) => {
                    if l != "'_" && !names.contains(l) {
                        names.push(l.clone());
                    }
                    collect(inner, names);
                }
                EType::Ref(None, t)
                | EType::Boxed(t)
                | EType::Buffer(t)
                | EType::Array(t, _)
                | EType::Slice(t) => collect(t, names),
                EType::Tuple(fields) => {
                    for t in fields {
                        collect(t, names);
                    }
                }
                EType::StructApplied(_, ls) | EType::EnumApplied(_, ls) => {
                    for l in ls {
                        if l != "'_" && !names.contains(l) {
                            names.push(l.clone());
                        }
                    }
                }
                EType::Fn(args, out) => {
                    for t in args {
                        collect(t, names);
                    }
                    collect(out, names);
                }
                _ => {}
            }
        }
        let mut names = Vec::new();
        for ty in types {
            collect(&ty, &mut names);
        }
        if names.is_empty() {
            String::new()
        } else {
            format!("<{}>", names.join(", "))
        }
    }
    let mut out = String::new();
    for item in &module.structs {
        if item.derives.contains(&Derive::Debug) {
            continue;
        }
        let ls = lifetimes(item.fields.iter().map(|(_, ty)| ty.clone()));
        out.push_str(&format!("impl{ls} std::fmt::Debug for {}{ls} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ f.debug_struct({:?})", item.name, item.name));
        for (name, _) in &item.fields {
            out.push_str(&format!(".field({name:?}, &self.{name})"));
        }
        out.push_str(".finish() } }\n");
    }
    for item in &module.enums {
        if item.derives.contains(&Derive::Debug) {
            continue;
        }
        let ls = lifetimes(item.variants.iter().flat_map(|v| v.payload.iter().cloned()));
        out.push_str(&format!("impl{ls} std::fmt::Debug for {}{ls} {{ fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{ match self {{", item.name));
        for variant in &item.variants {
            let fields: Vec<_> = (0..variant.payload.len())
                .map(|i| format!("field_{i}"))
                .collect();
            let pattern = match &variant.fields {
                Some(names) => format!(
                    " {{ {} }}",
                    names
                        .iter()
                        .zip(&fields)
                        .map(|(name, field)| format!("{name}: {field}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                None if fields.is_empty() => String::new(),
                None => format!("({})", fields.join(", ")),
            };
            out.push_str(&format!("Self::{}{pattern} => ", variant.name));
            if fields.is_empty() {
                out.push_str(&format!("f.write_str({:?}),", variant.name));
            } else if let Some(names) = &variant.fields {
                out.push_str(&format!("f.debug_struct({:?})", variant.name));
                for (name, field) in names.iter().zip(&fields) {
                    out.push_str(&format!(".field({name:?}, {field})"));
                }
                out.push_str(".finish(),");
            } else {
                out.push_str(&format!("f.debug_tuple({:?})", variant.name));
                for field in &fields {
                    out.push_str(&format!(".field({field})"));
                }
                out.push_str(".finish(),");
            }
        }
        out.push_str("} } }\n");
    }
    out
}

/// One Rust program for every accepted file. Each file's Rust is a module,
/// because names collide otherwise; the printer's header opens with inner
/// attributes, which a module may begin with as a crate may, so what the
/// printer allows (unused items among them) is allowed per file and nothing
/// is allowed for the harness as a whole.
pub fn harness(compiled: &[Compiled]) -> String {
    let mut source = String::from("// Generated by tests/corpus.rs. Do not edit.\n");
    // Each module answers its own run lines, from inside, since what a file
    // did not mark `pub` is private to its module; the index of a run line
    // counts across the files, in order. A call is written by the module's
    // path, as a remark shows it, and the root's items are imported so that
    // the path resolves from inside the module as it would from outside.
    let mut index = 0;
    for file in compiled {
        source.push_str(&format!("\n// {}\npub mod {} {{\n", file.file, file.module));
        source.push_str(&file.rust);
        source.push_str(&file.debug_support);
        if !file.runs.is_empty() {
            source.push_str("\n#[allow(unused_imports)]\nuse crate::*;\n");
            source.push_str("\npub fn answers(from: usize) {\n");
            for run in &file.runs {
                let answer = if run.lends() {
                    "answer_lending"
                } else {
                    "answer"
                };
                source.push_str(&format!(
                    "    crate::{answer}({index}, from, || {});\n",
                    run.call
                ));
                index += 1;
            }
            source.push_str("}\n");
        }
        source.push_str("}\n");
    }
    if index == 0 {
        source.push_str("\nfn main() {}\n");
        return source;
    }
    source.push_str(ANSWER);
    for file in compiled.iter().filter(|file| !file.runs.is_empty()) {
        source.push_str(&format!("    {}::answers(from);\n", file.module));
    }
    source.push_str("}\n");
    source
}

/// What one build answered against every run line, in order.
pub fn compare(compiled: &[Compiled], build: Overflow, observed: &[Seen]) -> Report {
    let mut report = Report::default();
    let mut observed = observed.iter();
    for file in compiled {
        for run in &file.runs {
            let remark = |message: String| Failure {
                file: file.file.clone(),
                line: run.line,
                message: format!("compiled Rust, {}: `{}` {message}", build.name(), run.call),
            };
            let Some(observed) = observed.next() else {
                report.failures.push(remark("was not answered".into()));
                continue;
            };
            match judge(run.expected_in(build), &run.lent_in(build), observed) {
                Verdict::Holds => {}
                Verdict::Fails(why) => report.failures.push(remark(why)),
                Verdict::Inconclusive(why) => report.inconclusive.push(remark(why)),
            }
        }
    }
    let extra = observed.count();
    if extra > 0 {
        report.failures.push(Failure {
            file: "the compiled program".into(),
            line: 0,
            message: format!(
                "{}: {extra} answer(s) more than there are run lines",
                build.name()
            ),
        });
    }
    report
}

/// How a run of the compiled program ended.
pub enum Ended {
    Exited(ExitStatus, String),
    /// Killed, having printed nothing for the length of the timeout.
    Killed,
}

/// Runs the program from a run line on, and returns the lines it printed.
pub fn run_from(binary: &Path, from: usize, timeout: Duration) -> (Vec<String>, Ended) {
    let mut child = Command::new(binary)
        .arg(from.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the program runs");
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let (send, receive) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if send.send(line).is_err() {
                break;
            }
        }
    });
    let errors = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let mut lines = Vec::new();
    let killed = loop {
        match receive.recv_timeout(timeout) {
            Ok(line) => lines.push(line),
            Err(RecvTimeoutError::Timeout) => break true,
            Err(RecvTimeoutError::Disconnected) => break false,
        }
    };
    if killed {
        let _ = child.kill();
    }
    let status = child.wait().expect("the program ends");
    // Whatever it printed between the timeout and being killed.
    reader.join().expect("the reader ends");
    lines.extend(receive.try_iter());
    let stderr = errors.join().expect("the reader ends");
    let ended = if killed {
        Ended::Killed
    } else {
        Ended::Exited(status, stderr)
    };
    (lines, ended)
}

/// Every run line's answer from one build. When the program stops short, the
/// run line it was in gets no answer, or an error if the program died, and
/// the program is started again from the next.
pub fn observe(binary: &Path, total: usize, timeout: Duration) -> Vec<Seen> {
    let mut observed = Vec::new();
    while observed.len() < total {
        let (lines, ended) = run_from(binary, observed.len(), timeout);
        observed.extend(lines.iter().map(|line| Observed::of_line(line)));
        if observed.len() >= total {
            break;
        }
        let failed = match ended {
            Ended::Killed => {
                Observed::NoAnswer(format!("killed after {timeout:?} without an answer"))
            }
            Ended::Exited(status, stderr) if !status.success() => {
                Observed::Failed(format!("the program exited with {status}: {stderr}"))
            }
            Ended::Exited(..) => Observed::Failed("the program printed nothing for it".into()),
        };
        observed.push((failed, Vec::new()));
    }
    observed
}

/// Compiles the harness once for each way of treating overflow, which is two
/// calls to rustc, runs each program, and compares. `name` keeps the files
/// of one batch apart from another's.
pub fn compile_and_compare(compiled: &[Compiled], name: &str, timeout: Duration) -> Report {
    let mut report = Report::default();
    let source = harness(compiled);
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source_path = directory.join(format!("{name}.rs"));
    std::fs::write(&source_path, &source).unwrap();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let total = compiled.iter().map(|file| file.runs.len()).sum();
    for build in Overflow::ALL {
        let binary_path = directory.join(format!("{name}_{build:?}").to_lowercase());
        let compile = Command::new(&rustc)
            .args([
                "--edition",
                "2021",
                "-D",
                "warnings",
                "-C",
                build.flag(),
                "-o",
            ])
            .arg(&binary_path)
            .arg(&source_path)
            .output()
            .expect("rustc runs");
        if !compile.status.success() {
            report.failures.push(Failure {
                file: "the compiled program".into(),
                line: 0,
                message: format!(
                    "{}: rustc rejected {}:\n{}",
                    build.name(),
                    source_path.display(),
                    String::from_utf8_lossy(&compile.stderr)
                ),
            });
            // The other build is of the same source.
            break;
        }
        let observed = observe(&binary_path, total, timeout);
        let compared = compare(compiled, build, &observed);
        report.failures.extend(compared.failures);
        report.inconclusive.extend(compared.inconclusive);
    }
    report
}

/// Inconclusive run lines are counted and printed whether or not the test
/// fails, past the capture of a passing test's output.
pub fn print_inconclusive(inconclusive: &[Failure]) {
    if inconclusive.is_empty() {
        return;
    }
    let listed: Vec<String> = inconclusive.iter().map(Failure::to_string).collect();
    let _ = writeln!(
        std::io::stderr(),
        "{} inconclusive comparison(s) in the corpus, neither passed nor failed:\n{}",
        inconclusive.len(),
        listed.join("\n")
    );
}

/// The `.lc` files of a directory of the repository, in order, with their text.
pub fn files_in(directory: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths: Vec<PathBuf> = std::fs::read_dir(root.join(directory))
        .unwrap_or_else(|error| panic!("{directory}: {error}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "lc"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            let text = std::fs::read_to_string(path).unwrap();
            (format!("{directory}/{name}"), text)
        })
        .collect()
}

/// Physical array and slice inputs use an array expression; vector inputs
/// own a Vec. The runtime comparison value itself only records contents.
pub fn rust_value_at_type(value: &Value, ty: &EType, module: &Module, path: &str) -> String {
    match (value, ty) {
        (Value::Tuple(fields), EType::Boxed(inner)) if fields.len() == 1 => format!(
            "Box::new({})",
            rust_value_at_type(&fields[0], inner, module, path)
        ),
        (value, EType::Ref(_, inner)) => {
            format!("&({})", rust_value_at_type(value, inner, module, path))
        }
        (
            Value::Buffer(items),
            EType::Array(element, _) | EType::Slice(element) | EType::Buffer(element),
        ) => {
            let items = items
                .iter()
                .map(|v| rust_value_at_type(v, element, module, path))
                .collect::<Vec<_>>()
                .join(", ");
            if matches!(ty, EType::Buffer(_)) {
                format!("vec![{items}]")
            } else {
                format!("[{items}]")
            }
        }
        (Value::Tuple(values), EType::Tuple(types)) => {
            let entries: Vec<_> = values
                .iter()
                .zip(types)
                .map(|(v, t)| rust_value_at_type(v, t, module, path))
                .collect();
            if entries.len() == 1 {
                format!("({},)", entries[0])
            } else {
                format!("({})", entries.join(", "))
            }
        }
        _ => rust_value(value, module, path),
    }
}
