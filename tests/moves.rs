//! Moves (O1), with rustc as the oracle. A program Locus rejects for a
//! runtime use of a moved value is elaborated once more with the move
//! analysis skipped (`elaborate_with(.., false)`, the test hook), printed as
//! Rust, and handed to rustc, which must reject it with E0382: Locus is
//! never more permissive than rustc about moves, and never rejects for a
//! reason rustc would not share. A program rejected only because a
//! proposition mentions a moved local has no counterpart in its Rust, where
//! the proposition is gone; its Rust must compile. The accepted shapes are
//! accepted, and are compiled by the corpus (`tests/corpus/accept/moves.lc`).

use std::path::PathBuf;
use std::process::Command;

use locus::diagnostic::Diagnostic;
use locus::elab::{Elaborated, elaborate_with};
use locus::erased::print_module;
use locus::parser::parse;
use locus::source::{SourceFile, SourceMap};

const PRELUDE: &str = "
struct Token {
    id: u8,
}

struct Pair {
    left: Token,
    right: Token,
    tag: u8,
}

enum Slot {
    Empty,
    Full(Token),
}

fn consume(t: Token) -> u8 {
    t.id
}

fn consume_pair(p: Pair) -> u8 {
    p.tag
}

fn consume_slot(s: Slot) -> u8 {
    match s {
        Slot::Empty => 0,
        Slot::Full(t) => t.id,
    }
}

fn peek(t: &Token) -> u8 {
    t.id
}

fn relabel(t: &mut Token, id: u8) -> () {
    t.id = id;
}
";

/// One program: its body after the prelude, the line (within the body,
/// from 1) and the message expected of the rejection, or `None` when it is
/// accepted.
struct Shape {
    name: &'static str,
    body: &'static str,
    rejection: Option<(usize, &'static str)>,
}

/// The programs rejected for a runtime use after a move: each is rejected
/// by rustc too.
const RUNTIME_REJECTIONS: &[Shape] = &[
    Shape {
        name: "let",
        body: "fn f(t: Token) -> u8 {
    let kept = t;
    consume(t)
}",
        rejection: Some((3, "use of moved value: `t`")),
    },
    Shape {
        name: "call",
        body: "fn f(t: Token) -> u8 {
    let first = consume(t);
    first.wrapping_add(consume(t))
}",
        rejection: Some((3, "use of moved value: `t`")),
    },
    Shape {
        name: "return",
        body: "fn f(t: Token) -> Token {
    let first = consume(t);
    t
}",
        rejection: Some((3, "use of moved value: `t`")),
    },
    Shape {
        name: "struct field",
        body: "fn f(t: Token) -> Pair {
    Pair { left: t, right: t, tag: 0 }
}",
        rejection: Some((2, "use of moved value: `t`")),
    },
    Shape {
        name: "tuple",
        body: "fn f(t: Token) -> (Token, Token) {
    (t, t)
}",
        rejection: Some((2, "use of moved value: `t`")),
    },
    Shape {
        name: "variant payload",
        body: "fn f(t: Token) -> Slot {
    let s = Slot::Full(t);
    Slot::Full(t)
}",
        rejection: Some((3, "use of moved value: `t`")),
    },
    Shape {
        name: "match binding",
        body: "fn f(s: Slot) -> u8 {
    let id = match s {
        Slot::Empty => 0u8,
        Slot::Full(t) => consume(t),
    };
    id.wrapping_add(consume_slot(s))
}",
        rejection: Some((6, "use of partially moved value: `s`")),
    },
    Shape {
        name: "one arm",
        body: "fn f(flag: bool, t: Token) -> u8 {
    let seen = if flag { consume(t) } else { 0 };
    seen.wrapping_add(consume(t))
}",
        rejection: Some((3, "use of moved value: `t`")),
    },
    Shape {
        name: "both arms",
        body: "fn f(flag: bool, t: Token) -> u8 {
    let seen = if flag { consume(t) } else { consume(t) };
    seen.wrapping_add(consume(t))
}",
        rejection: Some((3, "use of moved value: `t`")),
    },
    Shape {
        name: "loop body",
        body: "fn f(n: u8, t: Token) -> u8 {
    let mut total = 0u8;
    for i in 0..n {
        total = total.wrapping_add(consume(t));
    }
    total
}",
        rejection: Some((4, "use of moved value: `t`")),
    },
    Shape {
        name: "while body with continue",
        body: "fn f(n: u8) -> u8 {
    let mut t = Token { id: 1 };
    let mut i = 0u8;
    while i < n {
        i = i.wrapping_add(1);
        let seen = consume(t);
        if seen > 10 { continue; } else { t = Token { id: i }; }
    }
    i
}",
        rejection: Some((6, "use of moved value: `t`")),
    },
    Shape {
        name: "moved by break",
        body: "fn f(t: Token) -> u8 {
    let found = loop {
        break consume(t);
    };
    found.wrapping_add(consume(t))
}",
        rejection: Some((5, "use of moved value: `t`")),
    },
    Shape {
        name: "partial move then whole use",
        body: "fn f(p: Pair) -> u8 {
    let left = p.left;
    consume(left).wrapping_add(consume_pair(p))
}",
        rejection: Some((3, "use of partially moved value: `p`")),
    },
    Shape {
        name: "partial move then the same field",
        body: "fn f(p: Pair) -> u8 {
    let left = p.left;
    consume(left).wrapping_add(consume(p.left))
}",
        rejection: Some((3, "use of partially moved value: `p`")),
    },
    Shape {
        name: "lend after a move",
        body: "fn f(t: Token) -> u8 {
    let kept = t;
    peek(&t)
}",
        rejection: Some((3, "borrow of moved value: `t`")),
    },
    Shape {
        name: "lend by mutable reference after a move",
        body: "fn f(t: Token) -> u8 {
    let mut kept = t;
    let taken = kept;
    relabel(&mut kept, 2);
    0
}",
        rejection: Some((4, "borrow of moved value: `kept`")),
    },
    Shape {
        name: "assign to a field of a moved value",
        body: "fn f(t: Token) -> u8 {
    let mut kept = t;
    let seen = consume(kept);
    kept.id = 2;
    seen
}",
        rejection: Some((4, "assign to part of moved value: `kept`")),
    },
];

/// Rejected by Locus alone: the proposition is not in the Rust.
const PROPOSITION_REJECTIONS: &[Shape] = &[
    Shape {
        name: "prove! after a move",
        body: "fn f(n: u8) -> u8 {
    let t = Token { id: n };
    let out = consume(t);
    prove!(t.id == n);
    out
}",
        rejection: Some((4, "`t` was moved at line")),
    },
    Shape {
        name: "prop! after a move",
        body: "fn f(n: u8) -> Prop {
    let t = Token { id: n };
    let out = consume(t);
    prop!(t.id == out)
}",
        rejection: Some((4, "`t` was moved at line")),
    },
];

/// Accepted: what rustc accepts, Locus accepts.
const ACCEPTED: &[Shape] = &[
    Shape {
        name: "reinitialise then use",
        body: "fn f(a: u8, b: u8) -> u8 {
    let mut t = Token { id: a };
    let first = consume(t);
    t = Token { id: b };
    first.wrapping_add(consume(t))
}",
        rejection: None,
    },
    Shape {
        name: "reinitialised in a loop",
        body: "fn f(n: u8) -> u8 {
    let mut t = Token { id: 0 };
    for i in 0..n {
        let seen = consume(t);
        t = Token { id: i };
    }
    consume(t)
}",
        rejection: None,
    },
    Shape {
        name: "reinitialised in both arms",
        body: "fn f(flag: bool, t: Token) -> u8 {
    let mut t = t;
    if flag { t = Token { id: consume(t) }; } else { t = Token { id: 2 }; }
    consume(t)
}",
        rejection: None,
    },
    Shape {
        name: "Copy type used twice",
        body: "fn f(n: u8, pair: (u8, bool)) -> u8 {
    let m = n;
    let again = pair;
    n.wrapping_add(m).wrapping_add(pair.0).wrapping_add(again.0)
}",
        rejection: None,
    },
    Shape {
        name: "evidence used twice",
        body: "fn g(n: u8, small: @(n <= 10)) -> u8 { n }
fn f(n: u8, small: @(n <= 10)) -> u8 {
    let t = Token { id: n };
    let first = g(n, small);
    let out = consume(t);
    let second = g(n, small);
    first.wrapping_add(second).wrapping_add(out)
}",
        rejection: None,
    },
    Shape {
        name: "shadowing",
        body: "fn f(n: u8) -> u8 {
    let t = Token { id: n };
    let first = consume(t);
    let t = Token { id: first };
    consume(t)
}",
        rejection: None,
    },
    Shape {
        name: "partial move then another field",
        body: "fn f(p: Pair) -> u8 {
    let left = p.left;
    let tag = p.tag;
    consume(left).wrapping_add(consume(p.right)).wrapping_add(tag)
}",
        rejection: None,
    },
    Shape {
        name: "match binding nothing that moves",
        body: "fn f(s: Slot) -> u8 {
    let kind = match s {
        Slot::Empty => 0u8,
        Slot::Full(_) => 1,
    };
    kind.wrapping_add(consume_slot(s))
}",
        rejection: None,
    },
    Shape {
        name: "lent then moved",
        body: "fn f(t: Token) -> u8 {
    let seen = peek(&t);
    seen.wrapping_add(consume(t))
}",
        rejection: None,
    },
    Shape {
        name: "lent by mutable reference then moved",
        body: "fn f(t: Token) -> u8 {
    let mut kept = t;
    relabel(&mut kept, 2);
    relabel(&mut kept, 3);
    consume(kept)
}",
        rejection: None,
    },
    Shape {
        name: "a field of a reference parameter read",
        body: "fn f(p: &Pair) -> u8 {
    peek(&p.left).wrapping_add(p.tag)
}",
        rejection: None,
    },
    Shape {
        name: "proposition formed before the move",
        body: "fn f(n: u8) -> u8 {
    let t = Token { id: n };
    prove!(t.id == n);
    consume(t)
}",
        rejection: None,
    },
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
fn program(shape: &Shape) -> (String, usize) {
    let text = format!("{PRELUDE}\n{}\n", shape.body);
    // The prelude opens with a newline, so its lines are one more than
    // `lines` counts, and the body starts after the blank line.
    (text, PRELUDE.lines().count() + 2)
}

/// Locus rejects the shape with `code`, at the line and with the message
/// it says, and with no other diagnostic.
fn rejected_by_locus(shape: &Shape, code: &str) {
    let (text, first_line) = program(shape);
    let (result, sources) = elaborated(&text, true);
    let (line, message) = shape.rejection.expect("a rejected shape");
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
        "{}: expected {code} at line {} with `{message}`, found {found:#?}\n{text}",
        shape.name,
        first_line + line - 1
    );
    assert!(
        result.diagnostics.iter().all(|d| d.code == code),
        "{}: other diagnostics {found:#?}",
        shape.name
    );
}

/// The shape's Rust with the move analysis skipped, and what rustc said of it.
fn rustc_on(shape: &Shape) -> (bool, String) {
    let (text, _) = program(shape);
    let (result, _) = elaborated(&text, false);
    assert!(
        result.is_success(),
        "{}: rejected with the analysis skipped: {:#?}",
        shape.name,
        result.diagnostics
    );
    let source = print_module(result.session.erased());
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let file = shape.name.replace(' ', "_").replace('!', "");
    let source_path = directory.join(format!("moves_{file}.rs"));
    let output_path = directory.join(format!("libmoves_{file}.rlib"));
    std::fs::write(&source_path, &source).unwrap();
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
fn every_runtime_use_after_a_move_is_rejected_by_locus_and_by_rustc_with_e0382() {
    for shape in RUNTIME_REJECTIONS {
        rejected_by_locus(shape, "L0240");
        let (compiled, stderr) = rustc_on(shape);
        assert!(!compiled, "{}: rustc accepted the Rust", shape.name);
        assert!(
            stderr.contains("E0382"),
            "{}: rustc rejected the Rust for another reason:\n{stderr}",
            shape.name
        );
    }
}

#[test]
fn a_proposition_mentioning_a_moved_local_is_rejected_by_locus_alone() {
    for shape in PROPOSITION_REJECTIONS {
        rejected_by_locus(shape, "L0241");
        let (compiled, stderr) = rustc_on(shape);
        assert!(
            compiled,
            "{}: rustc rejected the Rust, which has no proposition:\n{stderr}",
            shape.name
        );
    }
}

#[test]
fn what_rustc_accepts_is_accepted() {
    for shape in ACCEPTED {
        let (text, _) = program(shape);
        let (result, _) = elaborated(&text, true);
        assert!(
            result.is_success(),
            "{}: rejected: {:#?}\n{text}",
            shape.name,
            result.diagnostics
        );
        let (compiled, stderr) = rustc_on(shape);
        assert!(
            compiled,
            "{}: rustc rejected the Rust:\n{stderr}",
            shape.name
        );
    }
}

#[test]
fn the_message_names_the_move() {
    let (text, first_line) = program(&RUNTIME_REJECTIONS[0]);
    let (result, sources) = elaborated(&text, true);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "L0240");
    assert_eq!(diagnostic.labels[0].message, "value used here after move");
    let moved = &diagnostic.labels[1];
    assert_eq!(moved.message, "value moved here");
    assert_eq!(line_of(&sources, diagnostic), first_line + 2);
    let moved_line = sources
        .get(moved.span.file)
        .line_column(moved.span.start)
        .unwrap()
        .0;
    assert_eq!(moved_line, first_line + 1);
    assert!(
        diagnostic.notes[0].contains("does not implement the `Copy` trait"),
        "{:?}",
        diagnostic.notes
    );
}

#[test]
fn a_move_in_a_loop_is_reported_at_the_move() {
    let shape = RUNTIME_REJECTIONS
        .iter()
        .find(|shape| shape.name == "loop body")
        .unwrap();
    let (text, first_line) = program(shape);
    let (result, sources) = elaborated(&text, true);
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(
        diagnostic.labels[0].message,
        "value moved here, in previous iteration of loop"
    );
    assert_eq!(line_of(&sources, diagnostic), first_line + 3);
}
