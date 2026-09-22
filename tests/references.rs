//! References as parameters (O3), with rustc as the oracle for the
//! aliasing of runtime values. Locus refuses two arguments of one call that
//! name overlapping places when either is lent by `&mut`, a `&mut` lend of
//! a binding that is not mutable, and a move out of a reference parameter,
//! each before rustc would and with rustc's code in the message; the Rust
//! of the same program must be refused by rustc with that code, so that
//! Locus is never more permissive than rustc, and never refuses for a
//! reason rustc would not share.
//!
//! The aliasing and mutability rules are checked in lowering too, which is
//! trusted and has no hook to skip, so the Rust of a rejected program is
//! made from the Rust of its accepted twin: the program with the arguments
//! kept apart, or the binding declared `mut`, is accepted and printed, and
//! the one call, or the one `let`, is rewritten as the rejected program
//! wrote it. A move out of a reference parameter is refused by the move
//! analysis alone, which the test hook `elaborate_with(.., false)` skips,
//! so its Rust is printed directly, as in `tests/moves.rs`.

use std::path::PathBuf;
use std::process::Command;

use locus::diagnostic::Diagnostic;
use locus::elab::{Elaborated, elaborate_with};
use locus::erased::print_module;
use locus::parser::parse;
use locus::source::{SourceFile, SourceMap};

const PRELUDE: &str = "
#[derive(Clone, Copy, Debug)]
struct Pair {
    lo: u8,
    hi: u8,
}

#[derive(Debug)]
struct Token {
    id: u8,
}

#[derive(Debug)]
struct Owned {
    left: Token,
    tag: u8,
}

enum Slot {
    Empty,
    Full(Token),
}

fn settle(a: &mut u8, b: &mut u8) -> () {
    a = b;
}

fn mixed(a: &mut Pair, b: &u8) -> () {
    a.lo = b;
}

fn mixed_back(a: &u8, b: &mut Pair) -> () {
    b.lo = a;
}

fn use_after(a: &mut Pair, b: u8) -> () {
    a.lo = b;
}

fn lend_then_move(a: &Token, b: Token) -> u8 {
    a.id.wrapping_add(b.id)
}

fn bump(counter: &mut u8) -> () {
    counter = counter.wrapping_add(1);
}

fn consume(t: Token) -> u8 {
    t.id
}
";

/// A program Locus rejects, with the line of its body (from 1) and what the
/// message must contain, and its accepted twin with the one rewrite that
/// turns the twin's Rust into the rejected program's.
struct Shape {
    name: &'static str,
    rejected: &'static str,
    line: usize,
    message: &'static str,
    rustc_code: &'static str,
    accepted: &'static str,
    /// In the accepted twin's Rust, this text is replaced by that.
    rewrite: (&'static str, &'static str),
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "two &mut lends of one place",
        rejected: "fn f(pair: Pair) -> Pair {
    let mut copy = pair;
    settle(&mut copy.lo, &mut copy.lo);
    copy
}",
        line: 3,
        message: "cannot borrow `copy.lo` as mutable more than once at a time (E0499)",
        rustc_code: "E0499",
        accepted: "fn f(pair: Pair) -> Pair {
    let mut copy = pair;
    settle(&mut copy.lo, &mut copy.hi);
    copy
}",
        rewrite: ("&mut copy.hi", "&mut copy.lo"),
    },
    Shape {
        name: "a &mut lend of the whole and a & lend of a field",
        rejected: "fn f(pair: Pair) -> Pair {
    let mut copy = pair;
    mixed(&mut copy, &copy.hi);
    copy
}",
        line: 3,
        message: "cannot borrow `copy.hi` as immutable because it is also borrowed as mutable (E0502)",
        rustc_code: "E0502",
        accepted: "fn f(pair: Pair, other: Pair) -> Pair {
    let mut copy = pair;
    mixed(&mut copy, &other.hi);
    copy
}",
        rewrite: ("&other.hi", "&copy.hi"),
    },
    Shape {
        name: "a & lend of a field and then a &mut lend of the whole",
        rejected: "fn f(pair: Pair) -> Pair {
    let mut copy = pair;
    mixed_back(&copy.hi, &mut copy);
    copy
}",
        line: 3,
        message: "cannot borrow `copy` as mutable because it is also borrowed as immutable (E0502)",
        rustc_code: "E0502",
        accepted: "fn f(pair: Pair, other: Pair) -> Pair {
    let mut copy = pair;
    mixed_back(&other.hi, &mut copy);
    copy
}",
        rewrite: ("&other.hi", "&copy.hi"),
    },
    Shape {
        name: "a read after a &mut lend of the whole",
        rejected: "fn f(pair: Pair) -> Pair {
    let mut copy = pair;
    use_after(&mut copy, copy.lo);
    copy
}",
        line: 3,
        message: "cannot use `copy.lo` because it was mutably borrowed (E0503)",
        rustc_code: "E0503",
        accepted: "fn f(pair: Pair, other: Pair) -> Pair {
    let mut copy = pair;
    use_after(&mut copy, other.lo);
    copy
}",
        rewrite: ("other.lo", "copy.lo"),
    },
    Shape {
        name: "a mention inside a later argument of a root lent by &mut",
        rejected: "fn f(pair: Pair) -> Pair {
    let mut copy = pair;
    use_after(&mut copy, copy.lo.wrapping_add(1));
    copy
}",
        line: 3,
        message: "cannot use `copy` because it was mutably borrowed (E0503)",
        rustc_code: "E0503",
        accepted: "fn f(pair: Pair, other: Pair) -> Pair {
    let mut copy = pair;
    use_after(&mut copy, other.lo.wrapping_add(1));
    copy
}",
        rewrite: ("other.lo.wrapping_add", "copy.lo.wrapping_add"),
    },
    Shape {
        name: "a move out of a place lent by &",
        rejected: "fn f(n: u8) -> u8 {
    let token = Token { id: n };
    lend_then_move(&token, token)
}",
        line: 3,
        message: "cannot move out of `token` because it is borrowed (E0505)",
        rustc_code: "E0505",
        accepted: "fn f(n: u8) -> u8 {
    let token = Token { id: n };
    let other = Token { id: n };
    lend_then_move(&token, other)
}",
        rewrite: (
            "lend_then_move(&token, other)",
            "lend_then_move(&token, token)",
        ),
    },
    Shape {
        name: "a &mut lend of an immutable binding",
        rejected: "fn f(n: u8) -> u8 {
    let count = n;
    bump(&mut count);
    count
}",
        line: 3,
        message: "cannot borrow `count` as mutable, as it is not declared as mutable (E0596)",
        rustc_code: "E0596",
        accepted: "fn f(n: u8) -> u8 {
    let mut count = n;
    bump(&mut count);
    count
}",
        rewrite: ("let mut count", "let count"),
    },
];

/// Refused by the move analysis: printed with the analysis skipped, and
/// refused by rustc with E0507.
const MOVES_OUT: &[(&str, &str, usize, &str)] = &[
    (
        "the whole of a & parameter",
        "fn f(t: &Token) -> Token {
    t
}",
        2,
        "cannot move out of `*t` which is behind a shared reference (E0507)",
    ),
    (
        "a field of a & parameter",
        "fn f(o: &Owned) -> Token {
    o.left
}",
        2,
        "cannot move out of `o.left` which is behind a shared reference (E0507)",
    ),
    (
        "the whole of a &mut parameter",
        "fn f(t: &mut Token) -> u8 {
    consume(t)
}",
        2,
        "cannot move out of `*t` which is behind a mutable reference (E0507)",
    ),
    (
        "a payload bound through a & parameter",
        "fn f(s: &Slot) -> u8 {
    match s {
        Slot::Empty => 0,
        Slot::Full(t) => consume(t),
    }
}",
        4,
        "cannot move out of `*s` which is behind a shared reference (E0507)",
    ),
];

fn elaborated(text: &str, check_moves: bool) -> (Elaborated, SourceMap) {
    let mut sources = SourceMap::default();
    let file = sources.add("shape.lc", text);
    let source: &SourceFile = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{text}\n{:#?}", parsed.diagnostics);
    let result = elaborate_with(source, &parsed.program, check_moves);
    (result, sources)
}

fn line_of(sources: &SourceMap, diagnostic: &Diagnostic) -> usize {
    let label = &diagnostic.labels[0];
    sources
        .get(label.span.file)
        .line_column(label.span.start)
        .map_or(0, |(line, _)| line)
}

/// The program's text, and the line its body starts at.
fn program(body: &str) -> (String, usize) {
    let text = format!("{PRELUDE}\n{body}\n");
    (text, PRELUDE.lines().count() + 2)
}

/// Locus rejects the body with `code` at its line, with the message, and
/// with no other diagnostic.
fn rejected_by_locus(name: &str, body: &str, code: &str, line: usize, message: &str) {
    let (text, first_line) = program(body);
    let (result, sources) = elaborated(&text, true);
    let found: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| format!("{} at {}: {}", d.code, line_of(&sources, d), d.message))
        .collect();
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == code
                && line_of(&sources, d) == first_line + line - 1
                && d.message.contains(message)
        }),
        "{name}: expected {code} at line {} with `{message}`, found {found:#?}\n{text}",
        first_line + line - 1
    );
    assert!(
        result.diagnostics.iter().all(|d| d.code == code),
        "{name}: other diagnostics {found:#?}"
    );
}

/// What rustc says of a Rust source, as a library.
fn rustc_on(name: &str, source: &str) -> (bool, String) {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let file = name.replace(' ', "_").replace('&', "ref");
    let source_path = directory.join(format!("references_{file}.rs"));
    let output_path = directory.join(format!("libreferences_{file}.rlib"));
    std::fs::write(&source_path, source).unwrap();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let compile = Command::new(rustc)
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "lib",
            "-D",
            "warnings",
            "-o",
        ])
        .arg(&output_path)
        .arg(&source_path)
        .output()
        .expect("rustc runs");
    (
        compile.status.success(),
        String::from_utf8_lossy(&compile.stderr).into_owned(),
    )
}

#[test]
fn every_aliasing_rejection_is_rustc_s_too() {
    for shape in SHAPES {
        let code = if shape.rustc_code == "E0596" {
            "L0262"
        } else {
            "L0263"
        };
        rejected_by_locus(shape.name, shape.rejected, code, shape.line, shape.message);
        // The twin is accepted, and its Rust compiles.
        let (text, _) = program(shape.accepted);
        let (result, _) = elaborated(&text, true);
        assert!(
            result.is_success(),
            "{}: the accepted twin was rejected: {:#?}",
            shape.name,
            result.diagnostics
        );
        let source = print_module(result.session.erased());
        let (compiled, stderr) = rustc_on(&format!("{} twin", shape.name), &source);
        assert!(
            compiled,
            "{}: rustc rejected the twin:\n{stderr}",
            shape.name
        );
        // Rewritten as the rejected program, rustc refuses it with the code.
        let (from, to) = shape.rewrite;
        assert!(
            source.contains(from),
            "{}: the twin's Rust does not contain `{from}`:\n{source}",
            shape.name
        );
        let rewritten = source.replacen(from, to, 1);
        let (compiled, stderr) = rustc_on(shape.name, &rewritten);
        assert!(!compiled, "{}: rustc accepted the Rust", shape.name);
        assert!(
            stderr.contains(shape.rustc_code),
            "{}: rustc rejected the Rust for another reason than {}:\n{stderr}",
            shape.name,
            shape.rustc_code
        );
    }
}

#[test]
fn every_move_out_of_a_reference_is_rejected_by_locus_and_by_rustc_with_e0507() {
    for (name, body, line, message) in MOVES_OUT {
        rejected_by_locus(name, body, "L0265", *line, message);
        let (text, _) = program(body);
        let (result, _) = elaborated(&text, false);
        assert!(
            result.is_success(),
            "{name}: rejected with the analysis skipped: {:#?}",
            result.diagnostics
        );
        let source = print_module(result.session.erased());
        let (compiled, stderr) = rustc_on(name, &source);
        assert!(!compiled, "{name}: rustc accepted the Rust:\n{source}");
        assert!(
            stderr.contains("E0507"),
            "{name}: rustc rejected the Rust for another reason:\n{stderr}"
        );
    }
}

/// A reference parameter is printed as declared, the value behind it is
/// read and written through `*`, and a lend of a reference parameter is a
/// reborrow.
#[test]
fn reference_parameters_print_as_rust_writes_them() {
    let (text, _) = program(
        "fn whole(pair: &Pair) -> Pair {
    pair
}

fn lends_on(pair: &mut Pair, other: &Pair) -> u8 {
    bump(&mut pair.lo);
    mixed(&mut pair, &other.hi);
    settle(&mut pair.lo, &mut pair.hi);
    pair.lo.wrapping_add(other.lo)
}

fn replaces(pair: &mut Pair) -> () {
    pair = Pair { lo: 0, hi: 0 };
}",
    );
    let (result, _) = elaborated(&text, true);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    let source = print_module(result.session.erased());
    for expected in [
        "fn whole(pair: &Pair) -> Pair {\n    *pair\n}",
        "fn lends_on(pair: &mut Pair, other: &Pair) -> u8 {",
        "bump(&mut pair.lo);",
        "mixed(&mut *pair, &other.hi);",
        "settle(&mut pair.lo, &mut pair.hi);",
        "pair.lo.wrapping_add(other.lo)",
        "fn replaces(pair: &mut Pair) -> () {\n    *pair = Pair { lo: 0_u8, hi: 0_u8 };\n}",
    ] {
        assert!(
            source.contains(expected),
            "missing `{expected}` in:\n{source}"
        );
    }
    let (compiled, stderr) = rustc_on("printed", &source);
    assert!(compiled, "rustc rejected the Rust:\n{stderr}");
}
