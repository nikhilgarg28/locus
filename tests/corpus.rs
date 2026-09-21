//! The corpus runner: a feature is tested by adding a `.lc` file.
//!
//! Every file in `examples`, `tests/corpus/accept`, and `tests/corpus/reject`
//! says what is expected of it in comments that start with `//~`:
//!
//! ~~~text
//! //~ proofs: 8                          the number of proofs found
//! //~ run: attempts_left(2, 9) => 1      a call and its result
//! //~ rust: pub fn run(n: u8) -> u8 {    text the generated Rust contains
//! //~ error: L0204                       an error reported on this line
//! //~^ error: L0204 unknown name         ... on the line above; `^^` is two
//!                                        above. Text after the code must
//!                                        appear in the message.
//! ~~~
//!
//! A file with an `error` directive must be rejected, with exactly the
//! errors it lists, each on its line. Any other file must be accepted: it is
//! parsed, elaborated, and checked, every run line is called in the check-IR
//! interpreter and in the erased-tree interpreter, and its Rust is printed.
//! The Rust of all accepted files goes into one source file, each in a `mod`
//! of its own, with a `main` that prints every run line's result; rustc runs
//! once, with `-D warnings`, and the output is compared with the same
//! expectations.
//!
//! Values in a run line are written as the interpreters print them, which is
//! also how Rust's `{:?}` prints them: `7`, `true`, `()`, `(1, Proved)`,
//! `Lock { failures: 0, open: false }`, `Wrong`, `NonZero(7, Proved)`. An
//! argument may name its enum, as in `Event::Wrong`; a result is compared as
//! text. `=> panic` is read, and reported as not supported until panics
//! exist.
//!
//! `examine` takes a file's name and text and returns every failure in it,
//! not the first, so the runner is tested on itself below with files whose
//! expectations are wrong.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;

use locus::diagnostic::Diagnostic;
use locus::elab::elaborate;
use locus::erased::{EType, Interpreter, Module, RunError, Value, check_module, print_module};
use locus::exec::CheckInterpreter;
use locus::parser::parse;
use locus::source::{SourceFile, SourceMap};

/// Steps an interpreter may take on one run line.
const FUEL: u64 = 10_000_000;

/// One expectation that did not hold. Line 0 means the file as a whole.
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
    Panic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Directive {
    Proofs(usize),
    Run {
        call: String,
        expected: Expected,
    },
    Rust(String),
    /// The line is the one the error is expected on, not the directive's.
    Error {
        line: usize,
        code: String,
        message: String,
    },
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
                Some((call, expected)) => Ok(Directive::Run {
                    call: call.trim().into(),
                    expected: match expected.trim() {
                        "panic" => Expected::Panic,
                        other => Expected::Value(other.into()),
                    },
                }),
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
                "unknown directive `{other}`; there are `proofs`, `run`, `rust`, and `error`"
            )),
        };
        found.push((number, directive));
    }
    found
}

/// Whether the file says it must be rejected.
fn expects_rejection(text: &str) -> bool {
    directives(text)
        .iter()
        .any(|(_, directive)| matches!(directive, Ok(Directive::Error { .. })))
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
    expected: String,
}

struct Examined {
    failures: Vec<Failure>,
    /// Present when the file was accepted.
    compiled: Option<Compiled>,
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
        compiled: None,
    })
}

fn examine_inner(name: &str, text: &str) -> Examined {
    let mut failures = Vec::new();
    let mut fail = |line: usize, message: String| {
        failures.push(Failure {
            file: name.into(),
            line,
            message,
        });
    };
    let mut found = Vec::new();
    for (line, directive) in directives(text) {
        match directive {
            Ok(directive) => found.push((line, directive)),
            Err(why) => fail(line, why),
        }
    }

    let mut sources = SourceMap::default();
    let file = sources.add(name, text);
    let source = sources.get(file);
    let parsed = parse(source);
    let elaborated = parsed
        .is_success()
        .then(|| elaborate(source, &parsed.program));
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
        if rejection && !matches!(directive, Directive::Error { .. }) {
            fail(
                *at,
                "a file with an `error` directive is not run, so this expects nothing".into(),
            );
        }
    }
    let Some(elaborated) = elaborated.filter(|elaborated| elaborated.is_success() && !rejection)
    else {
        return Examined {
            failures,
            compiled: None,
        };
    };

    let module = elaborated.session.erased();
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
                let proofs = elaborated.holes.len();
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
            Directive::Run { call, expected } => {
                let Expected::Value(expected) = expected else {
                    fail(at, "`=> panic` is not supported until panics exist".into());
                    continue;
                };
                let (function, arguments) = match parse_call(&call, module) {
                    Ok(parsed) => parsed,
                    Err(why) => {
                        fail(at, format!("`{call}`: {why}"));
                        continue;
                    }
                };
                // A function of the erased tree was accepted, so it has a reference.
                let reference = module.fns[function].reference;
                let shown = |result: Result<Value, RunError>| match result {
                    Ok(value) => value.debug(module),
                    Err(error) => format!("error: {error}"),
                };
                let results = [
                    (
                        "check-IR interpreter",
                        CheckInterpreter::new(elaborated.session.program(), FUEL)
                            .call(reference, arguments.clone()),
                    ),
                    (
                        "erased-tree interpreter",
                        Interpreter::new(module, FUEL).call(reference, arguments.clone()),
                    ),
                ];
                for (interpreter, result) in results {
                    let result = shown(result);
                    if result != expected {
                        fail(
                            at,
                            format!("{interpreter}: `{call}` is `{result}`, expected `{expected}`"),
                        );
                    }
                }
                let arguments: Vec<String> = arguments
                    .iter()
                    .map(|value| rust_value(value, module, &compiled.module))
                    .collect();
                compiled.runs.push(CompiledRun {
                    line: at,
                    call: format!(
                        "{}::{}({})",
                        compiled.module,
                        module.fns[function].name,
                        arguments.join(", ")
                    ),
                    expected,
                });
            }
            Directive::Error { .. } => {}
        }
    }
    Examined {
        failures,
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

    /// A value of a known type, so that `Wrong` needs no `Event::`.
    fn value(&mut self, ty: &EType, module: &Module) -> Result<Value, String> {
        match ty {
            EType::Bool => match self.word() {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                other => Err(format!("expected a `bool`, found `{other}`")),
            },
            EType::U8 => {
                let word = self.word();
                word.parse()
                    .map(Value::U8)
                    .map_err(|_| format!("expected a `u8`, found `{word}`"))
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
                self.expect("{")?;
                let mut values = Vec::new();
                for (index, (field, ty)) in item.fields.iter().enumerate() {
                    if index > 0 {
                        self.expect(",")?;
                    }
                    let word = self.word();
                    if word != field {
                        return Err(format!("expected field `{field}`, found `{word}`"));
                    }
                    self.expect(":")?;
                    values.push(self.value(ty, module)?);
                }
                self.eat(",");
                self.expect("}")?;
                Ok(Value::Struct(*id, values))
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
                let payload = &item.variants[index].payload;
                let values = if payload.is_empty() {
                    Vec::new()
                } else {
                    self.expect("(")?;
                    self.values(payload, module, ")")?
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
        Value::Bool(_) | Value::U8(_) => value.debug(module),
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
            let name = format!("{path}::{}::{}", item.name, item.variants[*index].name);
            if payload.is_empty() {
                name
            } else {
                format!("{name}({})", all(payload).join(", "))
            }
        }
    }
}

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
    source.push_str("\nfn main() {\n");
    for run in compiled.iter().flat_map(|file| &file.runs) {
        source.push_str(&format!("    println!(\"{{:?}}\", {});\n", run.call));
    }
    source.push_str("}\n");
    source
}

/// The compiled program prints one line for each run line, in order.
fn compare_output(compiled: &[Compiled], output: &str) -> Vec<Failure> {
    let mut failures = Vec::new();
    let mut lines = output.lines();
    for file in compiled {
        for run in &file.runs {
            let message = match lines.next() {
                Some(line) if line == run.expected => continue,
                Some(line) => format!(
                    "compiled Rust: `{}` is `{line}`, expected `{}`",
                    run.call, run.expected
                ),
                None => format!("compiled Rust: `{}` printed nothing", run.call),
            };
            failures.push(Failure {
                file: file.file.clone(),
                line: run.line,
                message,
            });
        }
    }
    let extra = lines.count();
    if extra > 0 {
        failures.push(Failure {
            file: "the compiled program".into(),
            line: 0,
            message: format!("printed {extra} line(s) more than there are run lines"),
        });
    }
    failures
}

/// Compiles the harness with one call to rustc, runs it, and compares.
fn compile_and_compare(compiled: &[Compiled]) -> Vec<Failure> {
    let whole = |message: String| {
        vec![Failure {
            file: "the compiled program".into(),
            line: 0,
            message,
        }]
    };
    let source = harness(compiled);
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source_path = directory.join("locus_corpus.rs");
    let binary_path = directory.join("locus_corpus");
    std::fs::write(&source_path, &source).unwrap();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let compile = Command::new(rustc)
        .args(["--edition", "2021", "-D", "warnings", "-o"])
        .arg(&binary_path)
        .arg(&source_path)
        .output()
        .expect("rustc runs");
    if !compile.status.success() {
        return whole(format!(
            "rustc rejected {}:\n{}",
            source_path.display(),
            String::from_utf8_lossy(&compile.stderr)
        ));
    }
    let run = Command::new(&binary_path)
        .output()
        .expect("the program runs");
    let mut failures = compare_output(compiled, &String::from_utf8_lossy(&run.stdout));
    if !run.status.success() {
        failures.extend(whole(format!(
            "exited with {}:\n{}",
            run.status,
            String::from_utf8_lossy(&run.stderr)
        )));
    }
    failures
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
    let mut compiled = Vec::new();
    let mut examples = Vec::new();
    for (directory, rejects) in [
        ("examples", false),
        ("tests/corpus/accept", false),
        ("tests/corpus/reject", true),
    ] {
        for (name, text) in files_in(directory) {
            if expects_rejection(&text) != rejects {
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
    failures.extend(compile_and_compare(&compiled));
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
fn increment(n: u8) -> (out: u8, @[out == n.wrapping_add(1)]) {
    (n.wrapping_add(1), _)
}
";

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
        [
            "4: check-IR interpreter: `increment(1)` is `(2, Proved)`, expected `(3, Proved)`",
            "4: erased-tree interpreter: `increment(1)` is `(2, Proved)`, expected `(3, Proved)`",
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
    assert_eq!(
        failures_of(&text),
        [
            "10: a run line reads `f(arguments) => value`",
            "12: unknown directive `prooofs`; there are `proofs`, `run`, `rust`, and `error`",
            "13: `^` belongs to `error`, not `run`",
            "4: 1 proof(s) were found, expected 2",
            "6: `increment(true)`: expected a `u8`, found `true`",
            "7: `increment(1, 2)`: more than 1 value(s) before `)`",
            "8: `decrement(1)`: there is no `decrement` in the erased tree; a function that exists only in proofs cannot be run",
            "9: `=> panic` is not supported until panics exist",
            "11: the generated Rust does not contain `pub fn decrement`",
        ]
    );
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
        compare_output(&compiled, output)
            .iter()
            .map(Failure::to_string)
            .collect()
    };
    assert_eq!(failures("(2, Proved)\n(3, Proved)\n"), Vec::<String>::new());
    assert_eq!(
        failures("(2, Proved)\n(4, Proved)\n"),
        [
            "memory.lc:5: compiled Rust: `memory::increment(2)` is `(4, Proved)`, expected `(3, Proved)`"
        ]
    );
    assert_eq!(
        failures("(2, Proved)\n"),
        ["memory.lc:5: compiled Rust: `memory::increment(2)` printed nothing"]
    );
    assert_eq!(
        failures("(2, Proved)\n(3, Proved)\n7\n"),
        ["the compiled program: printed 1 line(s) more than there are run lines"]
    );
    // The harness is one program: a module for the file, and the calls.
    let source = harness(&compiled);
    assert!(
        source.contains("mod memory {\n// Generated by Locus."),
        "{source}"
    );
    assert!(source.contains("    println!(\"{:?}\", memory::increment(2));\n"));
}
