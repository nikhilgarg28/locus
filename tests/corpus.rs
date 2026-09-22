//! The corpus runner: a feature is tested by adding a `.lc` file.
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
//! //~ rust: pub fn run(n: u8) -> u8 {    text the generated Rust contains
//! //~ error: L0204                       an error reported on this line
//! //~^ error: L0204 unknown name         ... on the line above; `^^` is two
//!                                        above. Text after the code must
//!                                        appear in the message.
//! //~ parse-only                         the file is parsed and nothing more
//! ~~~
//!
//! A file with an `error` directive must be rejected, with exactly the
//! errors it lists, each on its line. A file in `tests/corpus/target` says
//! `parse-only`: it is the target syntax, ahead of the elaborator, and only
//! the parser's diagnostics are compared with its `error` lines. Any other
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
//! also how Rust's `{:?}` prints them: `7`, `true`, `()`, `(1, Proved)`,
//! `Lock { failures: 0, open: false }`, `Wrong`, `NonZero(7, Proved)`. An
//! argument may name its enum, as in `Event::Wrong`; a result is compared as
//! text. A panic's message is written on one line, with `\n` for a newline
//! and `\\` for a backslash. The operators panic since E6, and `=> panic`
//! is also tested below on erased trees that were given a panic by hand.
//!
//! `examine` takes a file's name and text and returns every failure in it,
//! not the first, so the runner is tested on itself below with files whose
//! expectations are wrong.

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use locus::diagnostic::Diagnostic;
use locus::elab::elaborate;
use locus::erased::{
    self, EBlock, EExpr, EStmt, EType, Interpreter, Module, Outcome, RunError, Value, check_module,
    print_module,
};
use locus::exec::{CheckInterpreter, Program};
use locus::kernel::MachineInt;
use locus::parser::parse;
use locus::source::{SourceFile, SourceMap};

/// Steps an interpreter may take on one run line.
const FUEL: u64 = 10_000_000;

/// How long the compiled program may go without printing an answer before
/// it is killed and the run line it was in is inconclusive. Generous, since
/// being wrong about this costs a comparison and being slow costs nothing
/// when nothing hangs.
const TIMEOUT: Duration = Duration::from_secs(10);

/// One expectation that did not hold, or one that could not be decided. Line
/// 0 means the file as a whole.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
    file: String,
    line: usize,
    message: String,
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
enum Expected {
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

/// What a run line did, in an interpreter or in the compiled program.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Observed {
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
fn one_line(message: &str) -> String {
    message
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

impl Observed {
    fn of_interpreter(result: Result<Outcome, RunError>, module: &Module, fuel: u64) -> Self {
        match result {
            Ok(Outcome::Value(value)) => Self::Value(value.debug(module)),
            Ok(Outcome::Panic(message)) => Self::Panic(one_line(&message)),
            Ok(Outcome::OutOfFuel) => Self::NoAnswer(format!("out of fuel after {fuel} steps")),
            Err(error) => Self::Failed(error.to_string()),
        }
    }

    /// A line the compiled program printed. No value begins with `panic: `.
    fn of_line(line: &str) -> Self {
        match line.strip_prefix("panic: ") {
            Some(message) => Self::Panic(message.into()),
            None => Self::Value(line.into()),
        }
    }
}

enum Verdict {
    Holds,
    /// What is wrong, to follow the name of the call.
    Fails(String),
    /// Why nothing was learned, to follow the name of the call.
    Inconclusive(String),
}

/// An expectation against what was seen. Outcomes are compared as outcomes:
/// a value with a value, a panic with a panic and then the messages. No
/// answer is compared with nothing, so out of fuel can neither satisfy
/// `=> panic` nor contradict `=> 7`.
fn judge(expected: &Expected, observed: &Observed) -> Verdict {
    let holds = match (expected, observed) {
        (_, Observed::NoAnswer(_)) => return Verdict::Inconclusive(format!("gave {observed}")),
        (Expected::Value(expected), Observed::Value(value)) => expected == value,
        (Expected::Panic(None), Observed::Panic(_)) => true,
        (Expected::Panic(Some(expected)), Observed::Panic(message)) => expected == message,
        _ => false,
    };
    if holds {
        Verdict::Holds
    } else {
        Verdict::Fails(format!("is `{observed}`, expected `{expected}`"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Directive {
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
    /// The file is only parsed.
    ParseOnly,
}

/// The directives of a file, each with the line it is written on; a comment
/// that starts with `//~` and is not a directive is there with what is wrong.
fn directives(text: &str) -> Vec<(usize, Result<Directive, String>)> {
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
        if above > 0 && key != "error" {
            found.push((number, Err(format!("`^` belongs to `error`, not `{key}`"))));
            continue;
        }
        let directive = match key {
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
            "error" => {
                let (code, message) = value.split_once(' ').unwrap_or((value, ""));
                let is_code = code.len() == 5
                    && code.starts_with('L')
                    && code[1..].chars().all(|c| c.is_ascii_digit());
                if !is_code {
                    Err(format!("`{code}` is not an error code such as `L0204`"))
                } else if above >= number {
                    Err("there is no line that far above".into())
                } else {
                    Ok(Directive::Error {
                        line: number - above,
                        code: code.into(),
                        message: message.trim().into(),
                    })
                }
            }
            other => Err(format!(
                "unknown directive `{other}`; there are `proofs`, `run`, `rust`, `error`, and `parse-only`"
            )),
        };
        found.push((number, directive));
    }
    found
}

/// One outcome of a run line: a value, `panic`, or `panic: message`.
fn expectation(text: &str) -> Expected {
    match text.trim() {
        "panic" => Expected::Panic(None),
        other => match other.strip_prefix("panic:") {
            Some(message) => Expected::Panic(Some(message.trim().into())),
            None => Expected::Value(other.into()),
        },
    }
}

/// Whether the file says it must be rejected.
fn expects_rejection(text: &str) -> bool {
    directives(text)
        .iter()
        .any(|(_, directive)| matches!(directive, Ok(Directive::Error { .. })))
}

/// Whether the file says it is only parsed.
fn is_parse_only(text: &str) -> bool {
    directives(text)
        .iter()
        .any(|(_, directive)| matches!(directive, Ok(Directive::ParseOnly)))
}

/// An accepted file's part of the one program rustc compiles.
struct Compiled {
    file: String,
    module: String,
    rust: String,
    runs: Vec<CompiledRun>,
}

struct CompiledRun {
    line: usize,
    /// The call as a Rust expression, from outside the module.
    call: String,
    expected: Expected,
    /// The outcome in a build without overflow checks, when it differs.
    wrapping: Option<Expected>,
}

/// How the compiled program treats arithmetic overflow. The harness is built
/// once for each, because overflow is where a debug build and a release
/// build of the same Rust differ: a panic in one and wrapping in the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Overflow {
    Checked,
    Wrapping,
}

impl Overflow {
    const ALL: [Self; 2] = [Self::Checked, Self::Wrapping];

    fn flag(self) -> &'static str {
        match self {
            Self::Checked => "overflow-checks=on",
            Self::Wrapping => "overflow-checks=off",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Checked => "overflow checks on",
            Self::Wrapping => "overflow checks off",
        }
    }

    /// The interpreters' mode that matches the build.
    fn mode(self) -> erased::Overflow {
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
    fn expected_in(&self, build: Overflow) -> &Expected {
        match (build, &self.wrapping) {
            (Overflow::Wrapping, Some(wrapping)) => wrapping,
            _ => &self.expected,
        }
    }
}

/// What came of comparing: what is wrong, and what could not be decided.
#[derive(Default)]
struct Report {
    failures: Vec<Failure>,
    inconclusive: Vec<Failure>,
}

struct Examined {
    failures: Vec<Failure>,
    /// Run lines an interpreter ran out of fuel on. Not failures.
    inconclusive: Vec<Failure>,
    /// Present when the file was accepted.
    compiled: Option<Compiled>,
}

/// What the directives of an accepted file are checked against.
struct Subject<'a> {
    module: &'a Module,
    /// The check IR the erased tree is compared with. An erased tree that
    /// was built by hand has none, and runs in its own interpreter only.
    program: Option<&'a Program>,
    proofs: usize,
}

/// Everything wrong with one file, short of compiling its Rust. A panic
/// anywhere in the pipeline is one more failure, so the other files still
/// get their turn.
fn examine(name: &str, text: &str) -> Examined {
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
fn directives_of(name: &str, text: &str) -> (Vec<(usize, Directive)>, Vec<Failure>) {
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

fn examine_inner(name: &str, text: &str) -> Examined {
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
    let elaborated =
        (parsed.is_success() && !parse_only).then(|| elaborate(source, &parsed.program));
    let diagnostics = match &elaborated {
        Some(elaborated) => &elaborated.diagnostics,
        None => &parsed.diagnostics,
    };

    let reported: Vec<(usize, &Diagnostic)> = diagnostics
        .iter()
        .map(|diagnostic| (line_of(source, diagnostic), diagnostic))
        .collect();
    let rejection = expects_rejection(text);
    for (line, why) in compare_errors(&found, reported) {
        fail(line, why);
    }
    for (at, directive) in &found {
        if rejection && !matches!(directive, Directive::Error { .. } | Directive::ParseOnly) {
            fail(
                *at,
                "a file with an `error` directive is not run, so this expects nothing".into(),
            );
        } else if parse_only && !matches!(directive, Directive::Error { .. } | Directive::ParseOnly)
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
        proofs: elaborated.holes.len(),
    };
    let mut examined = examine_accepted(name, found, &subject, FUEL);
    failures.append(&mut examined.failures);
    examined.failures = failures;
    examined
}

/// An erased tree that did not come from its text, with the text's
/// directives: the way to a run line that panics while no source does.
fn examine_tree(name: &str, text: &str, module: &Module, fuel: u64) -> Examined {
    let (found, mut failures) = directives_of(name, text);
    let subject = Subject {
        module,
        program: None,
        proofs: 0,
    };
    let mut examined = examine_accepted(name, found, &subject, fuel);
    failures.append(&mut examined.failures);
    examined.failures = failures;
    examined
}

/// The directives of an accepted file against what it became: the erased
/// tree is type checked and printed, and every run line is called in each
/// interpreter there is.
fn examine_accepted(
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
    let rust = print_module(module);
    let mut compiled = Compiled {
        file: name.into(),
        module: module_name(name),
        rust,
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
                let (function, arguments) = match parse_call(&call, module) {
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
                };
                // Each interpreter in each mode, against the expectation
                // of the build the mode matches.
                for build in Overflow::ALL {
                    let results = [
                        subject.program.map(|program| {
                            (
                                "check-IR interpreter",
                                CheckInterpreter::new(program, fuel)
                                    .with_overflow(build.mode())
                                    .call(reference, arguments.clone()),
                            )
                        }),
                        Some((
                            "erased-tree interpreter",
                            Interpreter::new(module, fuel)
                                .with_overflow(build.mode())
                                .call(reference, arguments.clone()),
                        )),
                    ];
                    for (interpreter, result) in results.into_iter().flatten() {
                        let observed = Observed::of_interpreter(result, module, fuel);
                        let where_ = format!("{interpreter}, {}: `{call}`", build.name());
                        match judge(run.expected_in(build), &observed) {
                            Verdict::Holds => {}
                            Verdict::Fails(why) => fail(at, format!("{where_} {why}")),
                            Verdict::Inconclusive(why) => {
                                inconclusive.push(remark(at, format!("{where_} {why}")));
                            }
                        }
                    }
                }
                let arguments: Vec<String> = arguments
                    .iter()
                    .map(|value| rust_value(value, module, &compiled.module))
                    .collect();
                compiled.runs.push(CompiledRun {
                    call: format!(
                        "{}::{}({})",
                        compiled.module,
                        module.fns[function].name,
                        arguments.join(", ")
                    ),
                    ..run
                });
            }
            Directive::Error { .. } | Directive::ParseOnly => {}
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
fn compare_errors(
    found: &[(usize, Directive)],
    mut reported: Vec<(usize, &Diagnostic)>,
) -> Vec<(usize, String)> {
    let mut failures = Vec::new();
    for (_, directive) in found {
        let Directive::Error {
            line,
            code,
            message,
        } = directive
        else {
            continue;
        };
        let position = reported
            .iter()
            .position(|(on, diagnostic)| on == line && diagnostic.code == code);
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
                format!("expected error {code} on this line, and it was not reported"),
            )),
        }
    }
    for (line, diagnostic) in reported {
        failures.push((
            line,
            format!(
                "unexpected error {}: {}",
                diagnostic.code, diagnostic.message
            ),
        ));
    }
    failures
}

/// The line an error is reported at: where its primary label starts.
fn line_of(source: &SourceFile, diagnostic: &Diagnostic) -> usize {
    diagnostic
        .labels
        .iter()
        .find(|label| label.primary)
        .and_then(|label| source.line_column(label.span.start))
        .map_or(0, |(line, _)| line)
}

/// A Rust module name for a file: `examples/lock.lc` is `examples_lock`, and
/// `tests/corpus/accept/values.lc` is `accept_values`.
fn module_name(file: &str) -> String {
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
struct Cursor<'a> {
    rest: &'a str,
}

impl<'a> Cursor<'a> {
    fn eat(&mut self, token: &str) -> bool {
        self.rest = self.rest.trim_start();
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn expect(&mut self, token: &str) -> Result<(), String> {
        if self.eat(token) {
            Ok(())
        } else if self.rest.is_empty() {
            Err(format!("expected `{token}` at the end"))
        } else {
            Err(format!("expected `{token}` at `{}`", self.rest))
        }
    }

    /// A name or a number; empty when neither is next.
    fn word(&mut self) -> &'a str {
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
    fn values(
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
    fn fields(&mut self, fields: &[(&str, &EType)], module: &Module) -> Result<Vec<Value>, String> {
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
    fn value(&mut self, ty: &EType, module: &Module) -> Result<Value, String> {
        match ty {
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
                "Proved" => Ok(Value::Proved),
                other => Err(format!("expected `Proved`, found `{other}`")),
            },
            EType::Ghost => match self.word() {
                "Ghost" => Ok(Value::Ghost),
                other => Err(format!("expected `Ghost`, found `{other}`")),
            },
            EType::Tuple(tys) => {
                self.expect("(")?;
                Ok(Value::Tuple(self.values(tys, module, ")")?))
            }
            EType::Struct(id) => {
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
            EType::Enum(id) => {
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

/// `f(arguments)` as the function's index in the erased tree and the values.
fn parse_call(call: &str, module: &Module) -> Result<(usize, Vec<Value>), String> {
    let Some((name, arguments)) = call.split_once('(') else {
        return Err("a call reads `f(arguments)`".into());
    };
    let name = name.trim();
    let Some(index) = module.fns.iter().position(|f| f.name == name) else {
        return Err(format!(
            "there is no `{name}` in the erased tree; a function that exists only in proofs cannot be run"
        ));
    };
    let tys: Vec<EType> = module.fns[index]
        .params
        .iter()
        .map(|(_, _, ty)| ty.clone())
        .collect();
    let mut cursor = Cursor { rest: arguments };
    let values = cursor.values(&tys, module, ")")?;
    if !cursor.rest.trim().is_empty() {
        return Err(format!(
            "unexpected `{}` after the call",
            cursor.rest.trim()
        ));
    }
    Ok((index, values))
}

/// A value as a Rust expression, written from outside the file's module.
fn rust_value(value: &Value, module: &Module, path: &str) -> String {
    let all = |values: &[Value]| -> Vec<String> {
        values
            .iter()
            .map(|value| rust_value(value, module, path))
            .collect()
    };
    match value {
        // A bare number: the parameter's type fixes it, a negative one
        // included.
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
/// killed in.
const ANSWER: &str = r#"
fn answer<T: std::fmt::Debug>(index: usize, from: usize, call: fn() -> T) {
    use std::io::Write;
    if index < from {
        return;
    }
    let line = match std::panic::catch_unwind(call) {
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
    };
    let mut out = std::io::stdout().lock();
    writeln!(out, "{line}").and_then(|()| out.flush()).expect("stdout is open");
}

fn main() {
    // A panic is caught and answered with; nothing about it goes to stderr.
    std::panic::set_hook(Box::new(|_| {}));
    let from = std::env::args().nth(1).map_or(0, |from| from.parse().expect("an index"));
"#;

/// One Rust program for every accepted file. Each file's Rust is a module,
/// because names collide otherwise; the printer's header opens with inner
/// attributes, which a module may begin with as a crate may, so what the
/// printer allows (unused items among them) is allowed per file and nothing
/// is allowed for the harness as a whole.
fn harness(compiled: &[Compiled]) -> String {
    let mut source = String::from("// Generated by tests/corpus.rs. Do not edit.\n");
    for file in compiled {
        source.push_str(&format!("\n// {}\nmod {} {{\n", file.file, file.module));
        source.push_str(&file.rust);
        source.push_str("}\n");
    }
    let runs: Vec<&CompiledRun> = compiled.iter().flat_map(|file| &file.runs).collect();
    if runs.is_empty() {
        source.push_str("\nfn main() {}\n");
        return source;
    }
    source.push_str(ANSWER);
    for (index, run) in runs.iter().enumerate() {
        source.push_str(&format!("    answer({index}, from, || {});\n", run.call));
    }
    source.push_str("}\n");
    source
}

/// What one build answered against every run line, in order.
fn compare(compiled: &[Compiled], build: Overflow, observed: &[Observed]) -> Report {
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
            match judge(run.expected_in(build), observed) {
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
enum Ended {
    Exited(ExitStatus, String),
    /// Killed, having printed nothing for the length of the timeout.
    Killed,
}

/// Runs the program from a run line on, and returns the lines it printed.
fn run_from(binary: &Path, from: usize, timeout: Duration) -> (Vec<String>, Ended) {
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
fn observe(binary: &Path, total: usize, timeout: Duration) -> Vec<Observed> {
    let mut observed = Vec::new();
    while observed.len() < total {
        let (lines, ended) = run_from(binary, observed.len(), timeout);
        observed.extend(lines.iter().map(|line| Observed::of_line(line)));
        if observed.len() >= total {
            break;
        }
        observed.push(match ended {
            Ended::Killed => {
                Observed::NoAnswer(format!("killed after {timeout:?} without an answer"))
            }
            Ended::Exited(status, stderr) if !status.success() => {
                Observed::Failed(format!("the program exited with {status}: {stderr}"))
            }
            Ended::Exited(..) => Observed::Failed("the program printed nothing for it".into()),
        });
    }
    observed
}

/// Compiles the harness once for each way of treating overflow, which is two
/// calls to rustc, runs each program, and compares. `name` keeps the files
/// of one batch apart from another's.
fn compile_and_compare(compiled: &[Compiled], name: &str, timeout: Duration) -> Report {
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
fn print_inconclusive(inconclusive: &[Failure]) {
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
fn files_in(directory: &str) -> Vec<(String, String)> {
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

#[test]
fn every_file_is_checked_run_in_both_interpreters_compiled_and_compared() {
    let mut failures = Vec::new();
    let mut inconclusive = Vec::new();
    let mut compiled = Vec::new();
    let mut examples = Vec::new();
    for (directory, rejects, parse_only) in [
        ("examples", false, false),
        ("tests/corpus/accept", false, false),
        ("tests/corpus/reject", true, false),
        ("tests/corpus/target", false, true),
    ] {
        for (name, text) in files_in(directory) {
            if is_parse_only(&text) != parse_only {
                failures.push(Failure {
                    file: name.clone(),
                    line: 0,
                    message: if parse_only {
                        "a file in `target` needs a `parse-only` directive".into()
                    } else {
                        "a file with a `parse-only` directive belongs in `target`".into()
                    },
                });
            } else if !parse_only && expects_rejection(&text) != rejects {
                failures.push(Failure {
                    file: name.clone(),
                    line: 0,
                    message: if rejects {
                        "a file in `reject` needs an `error` directive".into()
                    } else {
                        "a file with an `error` directive belongs in `reject`".into()
                    },
                });
            }
            let examined = examine(&name, &text);
            failures.extend(examined.failures);
            inconclusive.extend(examined.inconclusive);
            compiled.extend(examined.compiled);
            if directory == "examples" {
                examples.push(name);
            }
        }
    }
    assert_eq!(
        examples,
        [
            "examples/increment.lc",
            "examples/lock.lc",
            "examples/preserve.lc",
            "examples/proofs.lc",
            "examples/propositions.lc",
        ]
    );
    let report = compile_and_compare(&compiled, "locus_corpus", TIMEOUT);
    failures.extend(report.failures);
    inconclusive.extend(report.inconclusive);
    print_inconclusive(&inconclusive);
    let listed: Vec<String> = failures.iter().map(Failure::to_string).collect();
    assert!(
        failures.is_empty(),
        "{} failure(s) in the corpus:\n{}",
        failures.len(),
        listed.join("\n")
    );
}

// The runner, tested on itself.

const INCREMENT: &str = "\
fn increment(n: u8) -> (out: u8, @(out == n.wrapping_add(1))) {
    (n.wrapping_add(1), _)
}
";

/// A failure of each interpreter in each mode, as the runner words them:
/// the interpreters first in one mode, then in the other.
fn in_each_interpreter(line: &str, rest: &str) -> Vec<String> {
    let mut messages = Vec::new();
    for build in Overflow::ALL {
        for interpreter in ["check-IR interpreter", "erased-tree interpreter"] {
            messages.push(format!("{line}: {interpreter}, {}: {rest}", build.name()));
        }
    }
    messages
}

/// The same for the erased-tree interpreter alone, as a tree built by hand
/// has no check IR.
fn in_erased_interpreter(line: &str, rest: &str) -> Vec<String> {
    Overflow::ALL
        .map(|build| format!("{line}: erased-tree interpreter, {}: {rest}", build.name()))
        .to_vec()
}

/// The failures of an in-memory file, as `line: message`.
fn failures_of(text: &str) -> Vec<String> {
    examine("memory.lc", text)
        .failures
        .iter()
        .map(|failure| format!("{}: {}", failure.line, failure.message))
        .collect()
}

#[test]
fn a_file_whose_expectations_hold_has_no_failures() {
    let text = format!(
        "{INCREMENT}//~ proofs: 1\n//~ run: increment(255) => (0, Proved)\n//~ rust: pub fn increment(n: u8) -> (u8, Proved) {{\n"
    );
    assert_eq!(failures_of(&text), Vec::<String>::new());
    let examined = examine("memory.lc", &text);
    let compiled = examined.compiled.expect("the file is accepted");
    assert_eq!(compiled.runs.len(), 1);
    assert_eq!(compiled.runs[0].call, "memory::increment(255)");
}

#[test]
fn a_wrong_expected_value_is_a_failure_in_each_interpreter() {
    let text = format!("{INCREMENT}//~ run: increment(1) => (3, Proved)\n");
    assert_eq!(
        failures_of(&text),
        in_each_interpreter(
            "4",
            "`increment(1)` is `(2, Proved)`, expected `(3, Proved)`"
        )
    );
}

const BUMP: &str = "\
fn bump(n: u8) -> u8 { n + 1 }
";

/// `=> panic | 0`: the outcome with overflow checks on, then with them
/// off. Each interpreter is judged in the mode of the build, and each
/// build against its own outcome.
#[test]
fn a_build_dependent_run_line_is_judged_per_build() {
    let text = format!(
        "{BUMP}//~ run: bump(255) => panic | 0
//~ run: bump(255) => panic: attempt to add with overflow | 0
//~ run: bump(254) => 255
//~ run: bump(255) => panic | 1
//~ run: bump(255) => 0 | 0
//~ run: bump(255) => panic
"
    );
    let wrong_wrapped = |line: &str| {
        Overflow::ALL
            .iter()
            .filter(|build| **build == Overflow::Wrapping)
            .flat_map(|build| {
                ["check-IR interpreter", "erased-tree interpreter"].map(|interpreter| {
                    format!(
                        "{line}: {interpreter}, {}: `bump(255)` is `0`, expected `1`",
                        build.name()
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    let mut expected = wrong_wrapped("5");
    for interpreter in ["check-IR interpreter", "erased-tree interpreter"] {
        expected.push(format!(
            "6: {interpreter}, overflow checks on: `bump(255)` is `panic: attempt to add with overflow`, expected `0`"
        ));
    }
    for interpreter in ["check-IR interpreter", "erased-tree interpreter"] {
        expected.push(format!(
            "7: {interpreter}, overflow checks off: `bump(255)` is `0`, expected `panic`"
        ));
    }
    assert_eq!(failures_of(&text), expected);

    // The compiled builds are compared with the same expectations.
    let compiled = [examine("memory.lc", &text).compiled.unwrap()];
    let runs = &compiled[0].runs;
    assert_eq!(
        runs[0].expected_in(Overflow::Checked),
        &Expected::Panic(None)
    );
    assert_eq!(
        runs[0].expected_in(Overflow::Wrapping),
        &Expected::Value("0".into())
    );
    assert_eq!(runs[2].expected_in(Overflow::Wrapping), &runs[2].expected);
    let checked_output = "panic: attempt to add with overflow\npanic: attempt to add with overflow\n255\npanic: attempt to add with overflow\npanic: attempt to add with overflow\npanic: attempt to add with overflow\n";
    let wrapping_output = "0\n0\n255\n0\n0\n0\n";
    let failures = |build: Overflow, output: &str| -> Vec<String> {
        let observed: Vec<Observed> = output.lines().map(Observed::of_line).collect();
        let report = compare(&compiled, build, &observed);
        report.failures.iter().map(Failure::to_string).collect()
    };
    assert_eq!(
        failures(Overflow::Checked, checked_output),
        [
            "memory.lc:6: compiled Rust, overflow checks on: `memory::bump(255)` is `panic: attempt to add with overflow`, expected `0`"
        ]
    );
    assert_eq!(
        failures(Overflow::Wrapping, wrapping_output),
        [
            "memory.lc:5: compiled Rust, overflow checks off: `memory::bump(255)` is `0`, expected `1`",
            "memory.lc:7: compiled Rust, overflow checks off: `memory::bump(255)` is `0`, expected `panic`",
        ]
    );
}

#[test]
fn a_missing_error_is_a_failure() {
    let text = format!("{INCREMENT}//~^ error: L0230\n");
    assert_eq!(
        failures_of(&text),
        ["3: expected error L0230 on this line, and it was not reported"]
    );
}

#[test]
fn an_error_on_the_wrong_line_is_a_failure_twice() {
    let text = "\
fn first(n: u8) -> u8 { //~ error: L0204
    missing
}
";
    assert_eq!(
        failures_of(text),
        [
            "1: expected error L0204 on this line, and it was not reported",
            "2: unexpected error L0204: unknown name `missing`",
        ]
    );
    // On its line, with the right code, the file has no failures; with the
    // wrong code or the wrong message it has.
    let text = "fn first(n: u8) -> u8 {\n    missing //~ error: L0204 unknown name\n}\n";
    assert_eq!(failures_of(text), Vec::<String>::new());
    assert_eq!(failures_of(&text.replace("L0204", "L0220")).len(), 2);
    assert_eq!(
        failures_of(&text.replace("unknown name", "unknown type")),
        ["2: L0204 says `unknown name `missing``, which does not contain `unknown type`"]
    );
}

#[test]
fn an_error_in_a_file_that_expects_none_is_a_failure() {
    let text = "fn first(n: u8) -> u8 {\n    missing\n}\n//~ run: first(1) => 1\n";
    assert_eq!(
        failures_of(text),
        ["2: unexpected error L0204: unknown name `missing`"]
    );
    assert!(examine("memory.lc", text).compiled.is_none());
}

#[test]
fn every_failure_in_a_file_is_reported() {
    let text = format!(
        "{INCREMENT}//~ proofs: 2
//~ run: increment(1) => (2, Proved)
//~ run: increment(true) => (2, Proved)
//~ run: increment(1, 2) => (2, Proved)
//~ run: decrement(1) => 0
//~ run: increment(1) => panic
//~ run: increment(1)
//~ rust: pub fn decrement
//~ prooofs: 1
//~^ run: increment(1) => (2, Proved)
"
    );
    let mut expected = vec![
        "10: a run line reads `f(arguments) => value`".to_string(),
        "12: unknown directive `prooofs`; there are `proofs`, `run`, `rust`, `error`, and `parse-only`".to_string(),
        "13: `^` belongs to `error`, not `run`".to_string(),
        "4: 1 proof(s) were found, expected 2".to_string(),
        "6: `increment(true)`: expected a `u8`, found `true`".to_string(),
        "7: `increment(1, 2)`: more than 1 value(s) before `)`".to_string(),
        "8: `decrement(1)`: there is no `decrement` in the erased tree; a function that exists only in proofs cannot be run".to_string(),
    ];
    expected.extend(in_each_interpreter(
        "9",
        "`increment(1)` is `(2, Proved)`, expected `panic`",
    ));
    expected.push("11: the generated Rust does not contain `pub fn decrement`".to_string());
    assert_eq!(failures_of(&text), expected);
}

#[test]
fn a_rejected_file_is_not_run() {
    let text = "fn first(n: u8) -> u8 {\n    missing //~ error: L0204\n}\n//~ run: first(1) => 1\n";
    assert_eq!(
        failures_of(text),
        ["4: a file with an `error` directive is not run, so this expects nothing"]
    );
}

#[test]
fn arguments_are_read_by_type_and_written_as_rust() {
    let text = "\
enum Event { Wrong, Right(u8, bool) }
struct Lock { failures: u8, open: bool }
fn pick(lock: Lock, event: Event, pair: (u8, (bool,)), unit: ()) -> Event { event }
//~ run: pick(Lock { failures: 1, open: false }, Right(2, true), (3, (false,)), ()) => Right(2, true)
//~ run: pick(Lock { failures: 1, open: false, }, Event::Wrong, (3, (false,),), (),) => Wrong
//~ run: pick(Lock { open: false, failures: 1 }, Wrong, (3, (false,)), ()) => Wrong
//~ run: pick(Lock { failures: 1, open: false }, Lock::Wrong, (3, (false,)), ()) => Wrong
//~ run: pick(Lock { failures: 1, open: false }, Middle, (3, (false,)), ()) => Wrong
//~ run: pick(Lock { failures: 1, open: false }, Wrong, (3, (false,)), ()) extra => Wrong
";
    assert_eq!(
        failures_of(text),
        [
            "6: `pick(Lock { open: false, failures: 1 }, Wrong, (3, (false,)), ())`: expected field `failures`, found `open`",
            "7: `pick(Lock { failures: 1, open: false }, Lock::Wrong, (3, (false,)), ())`: expected a `Event`, found `Lock`",
            "8: `pick(Lock { failures: 1, open: false }, Middle, (3, (false,)), ())`: `Event` has no variant `Middle`",
            "9: `pick(Lock { failures: 1, open: false }, Wrong, (3, (false,)), ()) extra`: unexpected `extra` after the call",
        ]
    );
    let compiled = examine("memory.lc", text).compiled.unwrap();
    assert_eq!(
        compiled.runs[0].call,
        "memory::pick(memory::Lock { failures: 1, open: false }, memory::Event::Right(2, true), (3, (false,)), ())"
    );
}

#[test]
fn compiled_output_that_differs_is_a_failure_on_its_run_line() {
    let text = format!(
        "{INCREMENT}//~ run: increment(1) => (2, Proved)\n//~ run: increment(2) => (3, Proved)\n"
    );
    let compiled = [examine("memory.lc", &text).compiled.unwrap()];
    let failures = |output: &str| -> Vec<String> {
        let observed: Vec<Observed> = output.lines().map(Observed::of_line).collect();
        let report = compare(&compiled, Overflow::Checked, &observed);
        assert!(report.inconclusive.is_empty());
        report.failures.iter().map(Failure::to_string).collect()
    };
    assert_eq!(failures("(2, Proved)\n(3, Proved)\n"), Vec::<String>::new());
    assert_eq!(
        failures("(2, Proved)\n(4, Proved)\n"),
        [
            "memory.lc:5: compiled Rust, overflow checks on: `memory::increment(2)` is `(4, Proved)`, expected `(3, Proved)`"
        ]
    );
    assert_eq!(
        failures("(2, Proved)\npanic: attempt to add with overflow\n"),
        [
            "memory.lc:5: compiled Rust, overflow checks on: `memory::increment(2)` is `panic: attempt to add with overflow`, expected `(3, Proved)`"
        ]
    );
    assert_eq!(
        failures("(2, Proved)\n"),
        ["memory.lc:5: compiled Rust, overflow checks on: `memory::increment(2)` was not answered"]
    );
    assert_eq!(
        failures("(2, Proved)\n(3, Proved)\n7\n"),
        ["the compiled program: overflow checks on: 1 answer(s) more than there are run lines"]
    );
    // The harness is one program: a module for the file, and the calls, each
    // under `catch_unwind`.
    let source = harness(&compiled);
    assert!(
        source.contains("mod memory {\n// Generated by Locus."),
        "{source}"
    );
    assert!(source.contains("    answer(1, from, || memory::increment(2));\n"));
    assert!(source.contains("std::panic::catch_unwind(call)"));
    // A batch with no run lines is a program all the same.
    assert!(harness(&[]).ends_with("fn main() {}\n"));
}

// Panics and returns. No source panics or returns yet, so the erased tree of
// a source is given them by hand: every call of `panics(k)` becomes a panic
// with message `k`, every call of `returns(e)` a `return e`, and every call
// of `returns_pair(k)` a `return (k, k)`.

const PANICS: &str = "\
enum Event { Wrong, Right(u8) }

fn panics(which: u8) -> u8 { which }

fn second(a: u8, b: u8) -> u8 { b }

fn in_let(n: u8) -> u8 {
    let m = panics(0);
    m
}

fn in_argument(n: u8) -> u8 { second(n, panics(1)) }

fn first_argument_first(n: u8) -> u8 { second(panics(2), panics(1)) }

fn in_tuple(n: u8) -> (u8, u8, u8) { (n, panics(3), n.wrapping_add(1)) }

fn in_arm(event: Event) -> u8 {
    match event {
        Event::Wrong => panics(4),
        Event::Right(n) => n,
    }
}

fn after_three(n: u8) -> u8 {
    let mut i: u8 = 0;
    loop {
        if i == n {
            break i
        } else {
            let _ = if i == 3 { panics(5) } else { i };
            i = i.wrapping_add(1);
        }
    }
}

fn at_the_top(n: u8) -> u8 {
    if n == 255 { panics(6) } else { n.wrapping_add(1) }
}

fn through_a_call(n: u8) -> u8 { second(at_the_top(n), 7) }
";

const MESSAGES: [&str; 7] = [
    "in a let",
    "in an argument",
    "the first argument",
    "in a tuple",
    "say \"hi\" to {n}, }{ and {{ and \\ too",
    "two\nlines",
    "f(255)",
];

const PANICS_RUNS: &str = r#"
//~ run: in_let(1) => panic: in a let
//~ run: in_argument(1) => panic: in an argument
//~ run: first_argument_first(1) => panic: the first argument
//~ run: in_tuple(1) => panic: in a tuple
//~ run: in_arm(Wrong) => panic: say "hi" to {n}, }{ and {{ and \\ too
//~ run: in_arm(Right(9)) => 9
//~ run: after_three(3) => 3
//~ run: after_three(4) => panic: two\nlines
//~ run: at_the_top(254) => 255
//~ run: at_the_top(255) => panic
//~ run: at_the_top(255) => panic: f(255)
//~ run: through_a_call(255) => panic: f(255)
//~ run: through_a_call(1) => 7
//~ rust: Event::Wrong => {
//~ rust: panic!("{}", "say \"hi\" to {n}, }{ and {{ and \\ too")
//~ rust: panic!("{}", "two\nlines")
"#;

/// The erased tree of `PANICS`, with its panics.
fn panicking_module() -> Module {
    planted_module(PANICS)
}

/// The erased tree of a source, with its panics and returns planted.
fn planted_module(text: &str) -> Module {
    let mut sources = SourceMap::default();
    let file = sources.add("planted.lc", text);
    let source = sources.get(file);
    let elaborated = elaborate(source, &parse(source).program);
    assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
    let mut module = elaborated.session.erased().clone();
    for function in &mut module.fns {
        plant_in_block(&mut function.body);
    }
    assert_eq!(check_module(&module), Ok(()));
    module
}

fn plant_in_block(block: &mut EBlock) {
    for stmt in &mut block.stmts {
        match stmt {
            EStmt::Let { value, .. } | EStmt::Assign { value, .. } => plant(value),
            EStmt::Expr(expr) => plant(expr),
        }
    }
    if let Some(tail) = &mut block.tail {
        plant(tail);
    }
}

fn plant(expr: &mut EExpr) {
    match expr {
        EExpr::Call {
            name, arguments, ..
        } if name == "returns" => {
            let [value] = arguments.as_mut_slice() else {
                panic!("`returns` takes one argument")
            };
            plant(value);
            *expr = EExpr::Return(Box::new(value.clone()));
        }
        EExpr::Call {
            name, arguments, ..
        } if name == "panics" || name == "returns_pair" => {
            let [EExpr::Literal(MachineInt::U8, which)] = arguments.as_slice() else {
                panic!("`{name}` takes a literal")
            };
            let which = *which;
            let index = usize::try_from(which).expect("a small byte");
            *expr = if name == "panics" {
                EExpr::Panic {
                    message: MESSAGES[index].into(),
                }
            } else {
                EExpr::Return(Box::new(EExpr::Tuple(vec![
                    EExpr::Literal(MachineInt::U8, which),
                    EExpr::Literal(MachineInt::U8, which),
                ])))
            };
        }
        EExpr::Var { .. }
        | EExpr::Bool(_)
        | EExpr::Literal(..)
        | EExpr::Proved
        | EExpr::Ghost
        | EExpr::Trap
        | EExpr::Panic { .. } => {}
        EExpr::Tuple(exprs)
        | EExpr::Variant { payload: exprs, .. }
        | EExpr::Call {
            arguments: exprs, ..
        }
        | EExpr::Operate {
            operands: exprs, ..
        } => exprs.iter_mut().for_each(plant),
        EExpr::Continue => {}
        EExpr::Break(inner) => inner.iter_mut().for_each(|inner| plant(inner)),
        EExpr::Struct { fields, .. } => fields.iter_mut().for_each(|(_, value)| plant(value)),
        EExpr::Field { target: inner, .. }
        | EExpr::Cast { expr: inner, .. }
        | EExpr::Return(inner) => {
            plant(inner);
        }
        EExpr::Method {
            receiver,
            arguments,
            ..
        } => {
            plant(receiver);
            arguments.iter_mut().for_each(plant);
        }
        EExpr::Compare { left, right, .. } => {
            plant(left);
            plant(right);
        }
        EExpr::If {
            condition,
            then_block,
            else_block,
        } => {
            plant(condition);
            plant_in_block(then_block);
            plant_in_block(else_block);
        }
        EExpr::Match {
            scrutinee, arms, ..
        } => {
            plant(scrutinee);
            arms.iter_mut()
                .for_each(|arm| plant_in_block(&mut arm.body));
        }
        EExpr::Block(block) | EExpr::Loop { body: block, .. } => plant_in_block(block),
        EExpr::While { condition, body } => {
            plant(condition);
            plant_in_block(body);
        }
        EExpr::For { lo, hi, body, .. } => {
            plant(lo);
            plant(hi);
            plant_in_block(body);
        }
    }
}

fn listed(remarks: &[Failure]) -> Vec<String> {
    remarks.iter().map(Failure::to_string).collect()
}

const RETURNS: &str = "\
enum Event { Wrong, Right(u8) }

fn returns(which: u8) -> u8 { which }

fn returns_pair(which: u8) -> (u8, u8) { (which, which) }

fn panics(which: u8) -> u8 { which }

fn after_a_let(n: u8) -> u8 {
    let m = panics(0);
    m.wrapping_add(1)
}

fn early(n: u8) -> u8 {
    let m = if n == 0 { returns(7) } else { n };
    m.wrapping_add(1)
}

fn from_a_loop(n: u8) -> u8 {
    let mut i: u8 = 0;
    loop {
        if i == n {
            break returns(i.wrapping_add(100))
        } else {
            i = i.wrapping_add(1);
        }
    }
}

fn from_a_for(n: u8) -> u8 {
    let mut last: u8 = 0;
    for i in 0..n {
        let _ = if i == 2 { returns(50) } else { i };
        last = i;
    }
    last
}

fn from_an_arm(event: Event) -> u8 {
    let (value, _) = match event {
        Event::Wrong => (returns(4), 0u8),
        Event::Right(n) => (n, n),
    };
    value
}

fn with_a_wildcard(n: u8) -> (u8, u8) {
    let (a, _) = if n == 0 { returns_pair(6) } else { returns_pair(8) };
    (a, 0)
}

fn through_a_call(n: u8) -> u8 { early(n).wrapping_add(10) }
";

const RETURNS_RUNS: &str = "\
//~ run: after_a_let(1) => panic: in a let
//~ run: early(0) => 7
//~ run: early(5) => 6
//~ run: from_a_loop(0) => 100
//~ run: from_a_loop(3) => 103
//~ run: from_a_for(2) => 1
//~ run: from_a_for(5) => 50
//~ run: from_an_arm(Wrong) => 4
//~ run: from_an_arm(Right(9)) => 9
//~ run: with_a_wildcard(0) => (6, 6)
//~ run: with_a_wildcard(1) => (8, 8)
//~ run: through_a_call(0) => 17
//~ run: through_a_call(5) => 16
//~ rust: let m: u8 = panic!(\"{}\", \"in a let\");
//~ rust: let m: u8 = if n == 0_u8 {
//~ rust: return 7_u8
//~ rust: break (return i.wrapping_add(100_u8))
//~ rust: let _ = if i == 2_u8 {
//~ rust: return 50_u8
//~ rust: let (value, _): (u8, _) = match event {
//~ rust: Event::Wrong => {
//~ rust: ((return 4_u8), 0_u8)
//~ rust: let (a, _): (u8, ()) = if n == 0_u8 {
//~ rust: return (6_u8, 6_u8)
";

#[test]
fn trees_that_return_agree_with_their_compiled_rust_and_a_let_bound_to_a_panic_has_a_type() {
    let module = planted_module(RETURNS);
    let text = format!("{RETURNS}{RETURNS_RUNS}");
    let examined = examine_tree("returns.lc", &text, &module, FUEL);
    assert_eq!(listed(&examined.failures), Vec::<String>::new());
    assert_eq!(listed(&examined.inconclusive), Vec::<String>::new());
    let report = compile_and_compare(
        &[examined.compiled.unwrap()],
        "locus_corpus_returns",
        TIMEOUT,
    );
    assert_eq!(listed(&report.failures), Vec::<String>::new());
    assert_eq!(listed(&report.inconclusive), Vec::<String>::new());
}

#[test]
fn trees_that_panic_agree_with_their_compiled_rust_message_included() {
    let module = panicking_module();
    let text = format!("{PANICS}{PANICS_RUNS}");
    let right = examine_tree("panics.lc", &text, &module, FUEL);
    assert_eq!(listed(&right.failures), Vec::<String>::new());
    assert_eq!(listed(&right.inconclusive), Vec::<String>::new());
    // The braces of a message are not the printer's: what follows the
    // message with more `{` than `}` is indented as it would be without it.
    let rust = &right.compiled.as_ref().unwrap().rust;
    assert!(
        rust.contains(
            "\n}\n\npub fn after_three(n: u8) -> u8 {\n    let mut i = 0_u8;\n    loop {\n"
        ),
        "{rust}"
    );

    // The same tree with expectations that are wrong in each way there is:
    // a panic where it returns, another message, and a value where it
    // panics. The interpreter says so, and so does each build.
    let wrong_runs = "\
//~ run: at_the_top(1) => panic
//~ run: at_the_top(255) => panic: f(254)
//~ run: at_the_top(255) => 0
//~ run: at_the_top(255) => panic: f(255)
";
    let wrong = examine_tree("wrong.lc", wrong_runs, &module, FUEL);
    assert_eq!(
        listed(&wrong.failures),
        [
            in_erased_interpreter("wrong.lc:1", "`at_the_top(1)` is `2`, expected `panic`"),
            in_erased_interpreter(
                "wrong.lc:2",
                "`at_the_top(255)` is `panic: f(255)`, expected `panic: f(254)`"
            ),
            in_erased_interpreter(
                "wrong.lc:3",
                "`at_the_top(255)` is `panic: f(255)`, expected `0`"
            ),
        ]
        .concat()
    );

    let compiled = [right.compiled.unwrap(), wrong.compiled.unwrap()];
    let report = compile_and_compare(&compiled, "locus_corpus_panics", TIMEOUT);
    assert_eq!(listed(&report.inconclusive), Vec::<String>::new());
    let expected: Vec<String> = ["overflow checks on", "overflow checks off"]
        .iter()
        .flat_map(|build| {
            [
                format!(
                    "wrong.lc:1: compiled Rust, {build}: `wrong::at_the_top(1)` is `2`, expected `panic`"
                ),
                format!(
                    "wrong.lc:2: compiled Rust, {build}: `wrong::at_the_top(255)` is `panic: f(255)`, expected `panic: f(254)`"
                ),
                format!(
                    "wrong.lc:3: compiled Rust, {build}: `wrong::at_the_top(255)` is `panic: f(255)`, expected `0`"
                ),
            ]
        })
        .collect();
    assert_eq!(listed(&report.failures), expected);
}

#[test]
fn out_of_fuel_is_inconclusive_and_is_not_a_panic() {
    // `after_three(200)` panics in its fourth iteration, and with fuel for
    // fewer it has done neither that nor anything else. Whatever the run
    // line expects, the answer is that there is none. A call that needs less
    // fuel is still compared.
    let module = panicking_module();
    let runs = "\
//~ run: after_three(200) => panic
//~ run: after_three(200) => panic: two\\nlines
//~ run: after_three(200) => 200
//~ run: at_the_top(1) => 2
";
    let examined = examine_tree("fuel.lc", runs, &module, 25);
    assert_eq!(listed(&examined.failures), Vec::<String>::new());
    let no_answer = "`after_three(200)` gave no answer: out of fuel after 25 steps";
    assert_eq!(
        listed(&examined.inconclusive),
        [
            in_erased_interpreter("fuel.lc:1", no_answer),
            in_erased_interpreter("fuel.lc:2", no_answer),
            in_erased_interpreter("fuel.lc:3", no_answer),
        ]
        .concat()
    );
    // With fuel, the first two hold and the third is wrong.
    let examined = examine_tree("fuel.lc", runs, &module, FUEL);
    assert_eq!(
        listed(&examined.failures),
        in_erased_interpreter(
            "fuel.lc:3",
            "`after_three(200)` is `panic: two\\nlines`, expected `200`"
        )
    );
    assert!(examined.inconclusive.is_empty());
}

#[test]
fn a_run_line_that_never_answers_is_inconclusive_and_the_rest_are_compared() {
    let text = "\
fn forever(n: u8) -> u8 {
    let mut i: u8 = n;
    loop {
        i = i.wrapping_add(1);
    }
}

fn next(n: u8) -> u8 { n.wrapping_add(1) }

//~ run: next(1) => 2
//~ run: forever(1) => 7
//~ run: next(2) => 3
//~ run: forever(2) => panic
//~ run: next(3) => 9
";
    // In the interpreters, out of fuel: inconclusive, whatever was expected.
    let (found, _) = directives_of("forever.lc", text);
    let mut sources = SourceMap::default();
    let file = sources.add("forever.lc", text);
    let source = sources.get(file);
    let elaborated = elaborate(source, &parse(source).program);
    assert!(elaborated.is_success(), "{:?}", elaborated.diagnostics);
    let subject = Subject {
        module: elaborated.session.erased(),
        program: Some(elaborated.session.program()),
        proofs: 0,
    };
    let examined = examine_accepted("forever.lc", found, &subject, 10_000);
    let by_interpreter = |line: usize, call: &str| {
        in_each_interpreter(
            &format!("forever.lc:{line}"),
            &format!("`{call}` gave no answer: out of fuel after 10000 steps"),
        )
    };
    assert_eq!(
        listed(&examined.inconclusive),
        [
            by_interpreter(11, "forever(1)"),
            by_interpreter(13, "forever(2)")
        ]
        .concat()
    );
    assert_eq!(
        listed(&examined.failures),
        in_each_interpreter("forever.lc:14", "`next(3)` is `4`, expected `9`")
    );

    // Compiled, each `forever` is killed at the timeout, which is short
    // here. It is inconclusive, and the run lines after it are compared: the
    // last one fails, in both builds.
    let timeout = Duration::from_secs(2);
    let report = compile_and_compare(
        &[examined.compiled.unwrap()],
        "locus_corpus_forever",
        timeout,
    );
    let builds = ["overflow checks on", "overflow checks off"];
    let killed = |build: &str, line: usize, call: &str| {
        format!(
            "forever.lc:{line}: compiled Rust, {build}: `{call}` gave no answer: killed after 2s without an answer"
        )
    };
    assert_eq!(
        listed(&report.inconclusive),
        builds
            .map(|build| {
                [
                    killed(build, 11, "forever::forever(1)"),
                    killed(build, 13, "forever::forever(2)"),
                ]
            })
            .concat()
    );
    assert_eq!(
        listed(&report.failures),
        builds.map(|build| format!(
            "forever.lc:14: compiled Rust, {build}: `forever::next(3)` is `4`, expected `9`"
        ))
    );
}
