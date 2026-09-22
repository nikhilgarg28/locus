//! Running printed Rust: one source file with a `mod` for every program, a
//! `main` that answers one line per call, one rustc call per way of treating
//! overflow, and a run that survives a call that hangs.
//!
//! This is the harness of `tests/corpus.rs` (E4), copied so that a second
//! test can use it: `Overflow`, `one_line`, `rust_value`, the text of
//! `ANSWER`, `run_from`, and `observe` are the same there and here, and
//! `harness` and `compile` are its `harness` and the rustc call inside its
//! `compile_and_compare`, with the corpus's run lines replaced by plain
//! calls. The corpus should come to use this file; until then a change to
//! one belongs in the other.
//!
//! This file is not part of `common/mod.rs`. A test includes it directly with
//! `#[path = "common/compiled.rs"] mod compiled;`.
// Each test that includes this file uses a different part of it.
#![allow(dead_code)]

use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use locus::erased::{Module, Value};

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

    fn flag(self) -> &'static str {
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
}

/// What the compiled program did with one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answered {
    /// As Rust's `{:?}` prints it, which is how `Value::debug` does.
    Value(String),
    /// The message, on one line as `one_line` writes it.
    Panic(String),
    /// Killed at the timeout. Not an outcome of the program: it may return,
    /// panic, or do neither.
    NoAnswer(String),
    /// The program died, or printed nothing for the call.
    Failed(String),
}

impl fmt::Display for Answered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(value) => f.write_str(value),
            Self::Panic(message) => write!(f, "panic: {message}"),
            Self::NoAnswer(why) => write!(f, "no answer: {why}"),
            Self::Failed(why) => write!(f, "error: {why}"),
        }
    }
}

impl Answered {
    /// A line the compiled program printed. No value begins with `panic: `.
    fn of_line(line: &str) -> Self {
        match line.strip_prefix("panic: ") {
            Some(message) => Self::Panic(message.into()),
            None => Self::Value(line.into()),
        }
    }
}

/// A panic's message as the one line it is printed on.
pub fn one_line(message: &str) -> String {
    message
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// A value as a Rust expression, written from outside the program's module.
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
        Value::Bool(_) | Value::Int(..) => value.debug(module),
        Value::Proved | Value::Ghost => format!("{path}::{}", value.debug(module)),
        Value::Tuple(fields) => match all(fields).as_slice() {
            [only] => format!("({only},)"),
            fields => format!("({})", fields.join(", ")),
        },
        Value::Struct(id, fields) => {
            let item = module.structs.iter().find(|item| item.id == *id);
            let item = item.expect("the value belongs to this module");
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
            let item = item.expect("the value belongs to this module");
            let name = format!("{path}::{}::{}", item.name, item.variants[*index].name);
            if payload.is_empty() {
                name
            } else {
                format!("{name}({})", all(payload).join(", "))
            }
        }
    }
}

/// What the harness answers a call with: its value as `{:?}` prints it, or
/// `panic: ` and the message of the panic it caught, on one line as
/// `one_line` writes it. Each answer is flushed, so that what was answered
/// before the program is killed is not lost, and the program starts from the
/// call it is given, so that it can be started again past the one it was
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

/// One program's part of the source rustc compiles.
pub struct Unit {
    /// The name of its `mod`.
    pub module: String,
    /// What `print_module` gave for it.
    pub rust: String,
    /// Each call as a Rust expression, from outside the module.
    pub calls: Vec<String>,
}

/// One Rust program for every unit. Each unit's Rust is a module, because
/// names collide otherwise; the printer's header opens with inner
/// attributes, which a module may begin with as a crate may, so what the
/// printer allows is allowed per unit and nothing is allowed for the harness
/// as a whole.
pub fn harness(units: &[Unit]) -> String {
    let mut source = String::from("// Generated by a test. Do not edit.\n");
    for unit in units {
        source.push_str(&format!("\nmod {} {{\n", unit.module));
        source.push_str(&unit.rust);
        source.push_str("}\n");
    }
    source.push_str(ANSWER);
    let calls = units.iter().flat_map(|unit| &unit.calls);
    for (index, call) in calls.enumerate() {
        source.push_str(&format!("    answer({index}, from, || {call});\n"));
    }
    source.push_str("}\n");
    source
}

/// The source's path and the binary's path for a name. `name` keeps the
/// files of one batch apart from another's.
pub fn paths(name: &str, build: Overflow) -> (PathBuf, PathBuf) {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let binary = directory.join(format!("{name}_{build:?}").to_lowercase());
    (directory.join(format!("{name}.rs")), binary)
}

/// Writes the source and compiles it, with `-D warnings`. The error is what
/// rustc wrote.
pub fn compile(name: &str, source: &str, build: Overflow) -> Result<PathBuf, String> {
    let (source_path, binary_path) = paths(name, build);
    std::fs::write(&source_path, source).expect("the source can be written");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
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
    if compile.status.success() {
        Ok(binary_path)
    } else {
        Err(String::from_utf8_lossy(&compile.stderr).into_owned())
    }
}

/// Removes the binaries `compile` built for a name, which are large. The
/// source stays, to be looked at, and is overwritten by the next run.
pub fn remove_binaries(name: &str) {
    for build in Overflow::ALL {
        let (_, binary) = paths(name, build);
        let _ = std::fs::remove_file(binary);
    }
}

/// How a run of the compiled program ended.
enum Ended {
    Exited(ExitStatus, String),
    /// Killed, having printed nothing for the length of the timeout.
    Killed,
}

/// Runs the program from a call on, and returns the lines it printed.
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

/// Every call's answer from one build. When the program stops short, the
/// call it was in gets no answer, or an error if the program died, and the
/// program is started again from the next.
pub fn observe(binary: &Path, total: usize, timeout: Duration) -> Vec<Answered> {
    let mut observed = Vec::new();
    while observed.len() < total {
        let (lines, ended) = run_from(binary, observed.len(), timeout);
        observed.extend(lines.iter().map(|line| Answered::of_line(line)));
        if observed.len() >= total {
            break;
        }
        observed.push(match ended {
            Ended::Killed => {
                Answered::NoAnswer(format!("killed after {timeout:?} without an answer"))
            }
            Ended::Exited(status, stderr) if !status.success() => {
                Answered::Failed(format!("the program exited with {status}: {stderr}"))
            }
            Ended::Exited(..) => Answered::Failed("the program printed nothing for it".into()),
        });
    }
    observed.truncate(total);
    observed
}
