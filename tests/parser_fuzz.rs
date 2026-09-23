//! Parser robustness on random input, and the progress guarantee.
//!
//! The two fuzz tests show what they tried and no more: on these inputs the
//! parser did not panic on a 1 MB stack and returned a program that accounts
//! for the input or at least one diagnostic. That the parser is total rests on
//! two guarantees tested on their own: every loop consumes a token or stops
//! (`steps_stay_linear_in_the_token_count` here, and the same bound on every
//! fuzz case), and recursion is bounded by `MAX_DEPTH` (the nesting test in
//! `tests/frontend.rs`).
//!
//! The seed is fixed, so a run is reproducible. A failure prints the seed of
//! its case, and `LOCUS_FUZZ_SEED=<seed> cargo test --test parser_fuzz <test>`
//! runs that case alone. A stack overflow aborts the process before anything
//! can be reported; `LOCUS_FUZZ_TRACE=1` with `--nocapture` prints every seed
//! before its case, so the last one printed is the one that overflowed.
//! `LOCUS_EXTENDED` (any value) multiplies the case counts by 100.

#[path = "common/rng.rs"]
mod rng;

use locus::lexer::lex;
use locus::parser::{ParseStats, parse};
use locus::source::SourceMap;
use rng::{Rng, case_seed};
use std::panic::{AssertUnwindSafe, catch_unwind};

const SEED: u64 = 0x4C4F_4355_5300_0002;
const TOKEN_SEQUENCES: u64 = 50_000;
const EDITED_FILES: u64 = 5_000;
const EXTENDED_FACTOR: u64 = 100;
const STACK_BYTES: usize = 1024 * 1024;

/// The progress bound: `steps <= STEP_FACTOR * tokens`, where the token count
/// includes the end-of-file token.
const STEP_FACTOR: usize = 4;

/// Every spelling the lexer gives a token kind of its own.
const TOKENS: &[&str] = &[
    "fn",
    "logic",
    "prop",
    "const",
    "let",
    "if",
    "else",
    "struct",
    "enum",
    "match",
    "loop",
    "for",
    "in",
    "break",
    "continue",
    "while",
    "return",
    "mut",
    "true",
    "false",
    "as",
    "_",
    "(",
    ")",
    "{",
    "}",
    "[",
    "]",
    ",",
    ":",
    ";",
    ".",
    "..",
    "::",
    "#",
    "@",
    "+",
    "!",
    "=",
    "==",
    "!=",
    "<",
    "<=",
    ">",
    ">=",
    "&&",
    "||",
    "->",
    "=>",
    "-",
    "*",
    "/",
    "%",
    "^",
    "&",
    "|",
    "<<",
    ">>",
    "+=",
    "-=",
    "*=",
    "/=",
    "%=",
    "^=",
    "&=",
    "|=",
    "<<=",
    ">>=",
    "...",
    "..=",
    "<-",
    "?",
    "$",
    "~",
    "/// doc\n",
    "//! doc\n",
    "/** doc */",
    "/*! doc */",
];

/// What Locus lexes because Rust has it: the keywords it reserves without
/// using them, and literals in every form, valid and not.
const RUST: &[&str] = &[
    "async",
    "await",
    "crate",
    "dyn",
    "extern",
    "impl",
    "mod",
    "move",
    "pub",
    "ref",
    "self",
    "Self",
    "static",
    "super",
    "trait",
    "type",
    "unsafe",
    "use",
    "where",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "gen",
    "macro",
    "override",
    "priv",
    "try",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "union",
    "macro_rules",
    "\"text\"",
    "\"\"",
    "\"a\\n\\t\\\\\\0\\x41\\u{1F4A1}\"",
    "\"two\nlines\"",
    "\"\\q\"",
    "\"\\x80\"",
    "\"\\u{110000}\"",
    "\"\\u{\"",
    "\"\\\n   \"",
    "b\"bytes\"",
    "b\"",
    "c\"text\"",
    "r\"raw\"",
    "r#\"raw\"#",
    "r##\"raw\"#",
    "br\"raw\"",
    "cr#\"raw\"#",
    "b'x'",
    "b'",
    "'a",
    "'static",
    "'_",
    "'x'",
    "'\\n'",
    "'\\''",
    "'ab'",
    "'💡'",
    "'\\",
    "0xff",
    "0xFF_u8",
    "0o377",
    "0b1010_1010",
    "0b102",
    "0x",
    "0x_",
    "0x1G",
    "255u8",
    "256u8",
    "65535u16",
    "1i128",
    "340282366920938463463374607431768211456u128",
    "1usize",
    "1u9",
    "1abc",
    "1.0",
    "1.",
    "1e5",
    "1E-5",
    "1e",
    "1.5f32",
    "2f64",
    "1.0abc",
    ".0",
    ".0.1",
];

/// Identifiers, including the contextual words, the names of the forms, the
/// names the examples use, and integers in valid and invalid spellings.
const WORDS: &[&str] = &[
    "math",
    "prop",
    "def",
    "forall",
    "exists",
    "prove",
    "rewrite",
    "unfold",
    "fold",
    "assert",
    "vec",
    "Prop",
    "u8",
    "bool",
    "x",
    "n",
    "f",
    "S",
    "E",
    "wrapping_add",
    "fn_name",
    "forall_",
    "__",
    "r",
    "0",
    "1",
    "255",
    "1_000",
    "99999999999999999999999999999999999999999",
    "0x1F",
    "1__0",
    "1_",
    "7u8",
    "_1",
];

/// Openers and fragments that lead the parser into each production, so that
/// random input reaches further than the first token of a declaration.
const FRAGMENTS: &[&str] = &[
    "fn f() -> u8 {",
    "#[terminates] #[no_panic] #[no_io] fn g(n: u8) -> Prop {",
    "fn h(x: u8, p: @(x == x)) -> (out: u8, @(out == x)) {",
    "struct S { x: u8, y: bool }",
    "struct S {",
    "enum E { A, B(u8), C(x: u8, bool) }",
    "enum E {",
    "prop P(n: u8) { Zero: @P(0), Next(m: u8, @P(m)): @(true) }",
    "prop P {",
    "const c: Prop = prop!(",
    "const k: u8 = 1;",
    "let x =",
    "let (a, _): (u8, bool) =",
    "let S { x, y: _ } =",
    "let E::B(v) =",
    "if c {",
    "} else {",
    "} else if c {",
    "match x {",
    "E::A =>",
    "E::B(v) =>",
    "S { x: 0, y } =>",
    "_ => 0,",
    "loop (s: u8 = 0) -> u8 {",
    "loop () -> u8 {",
    "for i in 0..n (s: u8 = 0) {",
    "for i in 0..n () {",
    "for i in f(",
    "for i in 0..g(n)(",
    "break",
    "continue(",
    "continue()",
    "forall (n: u8) {",
    "exists (n: u8, m: u8) {",
    "prop!(forall (n: u8) {",
    "prove!(exists (n: u8, m: u8) {",
    "prop!(",
    "prove!(",
    "prove!(n <= 3);",
    "@(",
    "@(x == x)",
    "@[",
    "@p",
    "[x <= 3]",
    "prop!(x <= 3)",
    "rewrite!(",
    "unfold!(same,",
    "fold!(nonzero, h)",
    "assert!(n < 3, \"small\")",
    "panic!(\"no room\")",
    "unreachable!()",
    "vec![1, 2]",
    "vec!(",
    "foo!(",
    "prove![",
    "todo!{",
    "!(",
    "x!",
    "S { x:",
    "E::B(",
    "f(",
    "x.0",
    ".wrapping_add(1)",
    "x: u8",
    "fn(u8, bool) -> u8",
    "math fn(x: u8) ->",
    "#[",
    "#![",
    "p => q =>",
    "a == b",
    "a < b < c",
    "!!",
    "#[derive(Debug, Clone)]",
    "#[cfg(",
    "#[terminates]",
    "#[terminates(decreases = n)]",
    "#[terminates(decreases =",
    "#[no_panic] #[no_alloc] #[no_io]",
    "#![no_panic]",
    "#[derive(Clone, std::marker::Copy)]",
    "#[derive]",
    "#[no_panic(x)]",
    "pub fn f(&self) -> u8 {",
    "pub(crate) fn f(self) -> u8 {",
    "pub(crate)",
    "pub(super)",
    "pub(in crate::a) struct S { pub x: u8 }",
    "pub(self)",
    "pub(in",
    "fn f<'a, T>(x: &'a T) -> T {",
    "impl S {",
    "impl S { fn f(&mut self) -> Self { Self { x: self.x } } }",
    "impl a::B {",
    "impl<T> S {",
    "impl T for S {",
    "self",
    "&self",
    "&mut self",
    "mut self",
    "Self",
    "Self::new(",
    "Self {",
    "::",
    "a::b::c",
    "crate::a",
    "super::a::B",
    "self::a",
    "u32::MAX",
    "{ a: 1 }",
    "E::V { a: 1, b }",
    "E::V { a, .. }",
    "S { x, .. } =>",
    "let E::V { a, b: _ } =",
    "..",
    "enum E { V { a: u8, b: bool }, W }",
    "enum E { V {",
    "Option<u8>",
    "Ghost<Option<u8>>",
    "-> Option<Percent> {",
    "x: Option<",
    "a as u8 < b",
    "/// doc\n",
    "//! doc\n",
    "/** doc */",
    "//// not doc\n",
    "use std::fmt;",
    "let mut x =",
    "x += 1;",
    "x as u8",
    "x as u8 as bool <",
    "-x as u8",
    "- -x",
    "a + b * c",
    "a - b - c",
    "a << b + c",
    "a & b == c",
    "a ^ b | c",
    "(a - b) - c",
    "a < b < c",
    "1 + (",
    "x..y",
    "..",
    "while x < 3 {",
    "return x;",
    "|y| y",
    "move ||",
    "fn move(ref: u8) -> u8 {",
    "S { type:",
    "x.await",
    "pair.0.1",
    "1.max(2)",
    "0..=n",
    // S5: `let mut`, assignment, `return`, Rust's loop forms, references,
    // and the never type, well formed and not.
    "let mut x = 1;",
    "let mut ok: @within_limit(lock.failures) =",
    "let (mut a, b) =",
    "let mut (a, b) =",
    "let mut _ =",
    "let mut mut y =",
    "let mut =",
    "x = 1;",
    "lock.failures = next;",
    "pair.0.x =",
    "f(x) = 1;",
    "x + 1 = 2;",
    "let y = x = 1;",
    "if x = 1 {",
    "(x = 1)",
    "x = ",
    "loop {",
    "loop { break; }",
    "loop { if done(n) { break; } n = n + 1; }",
    "while n < 3 {",
    "while let Some(v) = it.next() {",
    "while let (a, mut b) = p {",
    "while let",
    "while x = 1 {",
    "for (a, b) in pairs {",
    "for i in 0..=n {",
    "for x in items.iter() {",
    "for x in items {",
    "for E::V(x) in xs {",
    "for mut in",
    "for i in 0..f() {",
    "break;",
    "break (i, _)",
    "continue;",
    "continue",
    "return;",
    "return (x, x);",
    "return",
    "&x",
    "&mut x",
    "&mut lock.failures",
    "f(&mut a, &b)",
    "&&x",
    "&mut",
    "&",
    "x: &u8",
    "x: &mut Lock",
    "x: &&u8",
    "x: &mut (u8, u8)",
    "-> ! {",
    "-> &mut S {",
    "fn f(mut n: u8) -> u8 {",
    "fn f(mut: u8) -> u8 {",
    "prop P(mut n: u8) {",
    "forall (mut n: u8) {",
    "if c { } else { } (a, b)",
    "match x { _ => 1 } (1, 2)",
    "if c { a } else { b }.wrapping_add(1);",
    "{ a } if c { 1 } else { 2 }",
    "Some(x) =>",
    "let Some(x) =",
    "*x = 1;",
    "x += 1;",
];

/// Text the lexer must reject or skip rather than tokenize.
const JUNK: &[&str] = &[
    "//",
    "// note\n",
    "/*",
    "*/",
    "/* a /* b */",
    "\"",
    "\"a\\",
    "\"\\\"",
    "r#x",
    "r#",
    "\\",
    "'",
    "`",
    "é",
    "λx",
    "💡",
    "e\u{301}",
    "\u{0}",
    "\u{7f}",
    "\u{a0}",
    "\u{2028}",
    "\u{feff}",
    "\u{10ffff}",
    "\r",
    "\r\n",
    "\t",
    "\n",
];

fn extended() -> bool {
    std::env::var_os("LOCUS_EXTENDED").is_some_and(|value| !value.is_empty())
}

fn count(fast: u64) -> u64 {
    if extended() {
        fast * EXTENDED_FACTOR
    } else {
        fast
    }
}

/// The seed named by `LOCUS_FUZZ_SEED`, in decimal or in hexadecimal with `0x`.
fn replayed_seed() -> Option<u64> {
    let text = std::env::var("LOCUS_FUZZ_SEED").ok()?;
    let text = text.trim();
    let seed = match text.strip_prefix("0x") {
        Some(digits) => u64::from_str_radix(&digits.replace('_', ""), 16),
        None => text.parse(),
    };
    Some(seed.unwrap_or_else(|_| panic!("LOCUS_FUZZ_SEED is not a 64-bit seed: {text:?}")))
}

/// What one parse is checked for. `Err` says which property failed.
fn examine(text: &str) -> Result<ParseStats, String> {
    let mut sources = SourceMap::default();
    let file = sources.add("fuzz.lc", text);
    let source = sources.get(file);
    let parsed = parse(source);

    let stats = parsed.stats;
    if stats.steps > STEP_FACTOR * stats.tokens {
        return Err(format!(
            "the parser took {} steps on {} tokens, above the bound of {STEP_FACTOR} per token",
            stats.steps, stats.tokens
        ));
    }
    for diagnostic in &parsed.diagnostics {
        if diagnostic.labels.is_empty() {
            return Err(format!("{} has no label", diagnostic.code));
        }
        for label in &diagnostic.labels {
            if source.slice(label.span).is_none() {
                return Err(format!(
                    "{} has a label outside the source or inside a character: {:?}",
                    diagnostic.code, label.span
                ));
            }
        }
    }
    // An AST or a diagnostic, never neither: a silent parse has to account
    // for every token with a declaration, or with the file's own doc
    // comments and attributes.
    if parsed.diagnostics.is_empty() {
        let program = &parsed.program;
        let spans: Vec<_> = program
            .declarations
            .iter()
            .map(|declaration| declaration.span)
            .chain(program.doc.iter().map(|doc| doc.span))
            .chain(program.attributes.iter().map(|attribute| attribute.span))
            .collect();
        for token in lex(source).tokens {
            let covered = token.span.start == token.span.end
                || spans
                    .iter()
                    .any(|span| span.start <= token.span.start && token.span.end <= span.end);
            if !covered {
                return Err(format!(
                    "no diagnostic, yet no declaration covers the token at {:?}",
                    token.span
                ));
            }
        }
    }
    Ok(stats)
}

/// Runs the cases on one thread with a 1 MB stack. A panic inside the parser
/// is caught per case, so the report can name the seed and the input.
fn run(test: &'static str, cases: u64, generate: impl Fn(&mut Rng) -> String + Send + 'static) {
    let seeds: Box<dyn Iterator<Item = u64> + Send> = match replayed_seed() {
        Some(seed) => Box::new(std::iter::once(seed)),
        None => Box::new((0..count(cases)).map(|index| case_seed(SEED, index))),
    };
    let trace = std::env::var_os("LOCUS_FUZZ_TRACE").is_some();
    let outcome = std::thread::Builder::new()
        .name(test.into())
        .stack_size(STACK_BYTES)
        .spawn(move || {
            let mut worst = 0.0f64;
            for seed in seeds {
                if trace {
                    println!("{test}: seed {seed:#018x}");
                }
                let text = generate(&mut Rng::new(seed));
                let failure = match catch_unwind(AssertUnwindSafe(|| examine(&text))) {
                    Ok(Ok(stats)) => {
                        worst = worst.max(stats.steps as f64 / stats.tokens as f64);
                        continue;
                    }
                    Ok(Err(reason)) => reason,
                    Err(_) => "the parser panicked (its message is above)".to_owned(),
                };
                return Err(format!(
                    "{failure}\n  seed:   {seed:#018x}\n  replay: LOCUS_FUZZ_SEED={seed:#x} cargo test --test parser_fuzz {test}\n  input:  {text:?}"
                ));
            }
            println!("{test}: most steps per token: {worst:.3}");
            Ok(())
        })
        .unwrap()
        .join()
        .expect("the fuzz thread itself panicked");
    if let Err(report) = outcome {
        panic!("{report}");
    }
}

fn junk_character(rng: &mut Rng) -> char {
    let limit = *rng.choose(&[0x80, 0x800, 0x1_0000, 0x11_0000]);
    // The surrogates are not characters.
    char::from_u32(rng.below(limit) as u32).unwrap_or('\u{fffd}')
}

fn piece(rng: &mut Rng, text: &mut String) {
    let vocabulary = match rng.below(100) {
        0..50 => TOKENS,
        50..62 => WORDS,
        62..72 => RUST,
        72..92 => FRAGMENTS,
        92..97 => JUNK,
        _ => return text.push(junk_character(rng)),
    };
    let spelling = *rng.choose(vocabulary);
    text.push_str(spelling);
}

/// Source text joined from the token vocabulary. Most pieces are separated by
/// a space; some are not, so that spellings also fuse (`=` `=`, `1` `x`).
/// Now and then one piece repeats many times, which is how random input gets
/// deep enough to meet the nesting limit.
fn token_sequence(rng: &mut Rng) -> String {
    let pieces = if rng.chance(1, 50) {
        rng.range(0..400)
    } else {
        rng.range(0..48)
    };
    let mut text = String::new();
    for _ in 0..pieces {
        if rng.chance(1, 40) {
            let mut repeated = String::new();
            piece(rng, &mut repeated);
            repeated.push(' ');
            text.push_str(&repeated.repeat(rng.range(2..200)));
        } else {
            piece(rng, &mut text);
        }
        if !rng.chance(1, 8) {
            text.push(if rng.chance(1, 10) { '\n' } else { ' ' });
        }
    }
    text
}

/// The tokens of every example, in file name order.
fn example_tokens() -> Vec<Vec<String>> {
    let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "lc"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no examples in {directory}");
    paths
        .iter()
        .map(|path| {
            let mut sources = SourceMap::default();
            let file = sources.add("example.lc", std::fs::read_to_string(path).unwrap());
            let source = sources.get(file);
            let lexed = lex(source);
            assert!(lexed.diagnostics.is_empty(), "{}", path.display());
            lexed
                .tokens
                .iter()
                .map(|token| source.slice(token.span).unwrap().to_owned())
                .filter(|spelling| !spelling.is_empty())
                .collect()
        })
        .collect()
}

/// One example with one to four edits: delete a token, duplicate a token,
/// swap two tokens, or insert a token from the vocabulary.
fn edited_file(rng: &mut Rng, examples: &[Vec<String>]) -> String {
    let mut tokens = rng.choose(examples).clone();
    let edits = if rng.chance(3, 4) { 1 } else { rng.range(2..5) };
    for _ in 0..edits {
        if tokens.is_empty() {
            break;
        }
        let at = rng.range(0..tokens.len());
        match rng.below(4) {
            0 => {
                tokens.remove(at);
            }
            1 => tokens.insert(at, tokens[at].clone()),
            2 => {
                // Half the swaps are of neighbours, the likelier slip.
                let other = if rng.chance(1, 2) {
                    (at + 1).min(tokens.len() - 1)
                } else {
                    rng.range(0..tokens.len())
                };
                tokens.swap(at, other);
            }
            _ => tokens.insert(at, (*rng.choose(TOKENS)).to_owned()),
        }
    }
    tokens.join(" ")
}

#[test]
fn random_token_sequences_give_an_ast_or_a_diagnostic_without_a_panic() {
    run(
        "random_token_sequences_give_an_ast_or_a_diagnostic_without_a_panic",
        TOKEN_SEQUENCES,
        token_sequence,
    );
}

#[test]
fn edited_examples_give_an_ast_or_a_diagnostic_without_a_panic() {
    let examples = example_tokens();
    run(
        "edited_examples_give_an_ast_or_a_diagnostic_without_a_panic",
        EDITED_FILES,
        move |rng| edited_file(rng, &examples),
    );
}

/// The progress guarantee on its own: every parser loop consumes a token or
/// stops, so the step count is linear in the token count. The inputs are the
/// examples, which parse, and the shapes that would expose a loop that stalls
/// or a lookahead that rescans: long runs of one token, recovery that starts
/// over and over, and `for` headers whose state list lookahead never closes.
#[test]
fn steps_stay_linear_in_the_token_count() {
    let mut inputs: Vec<String> = example_tokens()
        .iter()
        .map(|tokens| tokens.join(" "))
        .collect();
    for spelling in TOKENS
        .iter()
        .chain(WORDS)
        .chain(RUST)
        .chain(FRAGMENTS)
        .chain(JUNK)
    {
        inputs.push(format!("{spelling} ").repeat(2_000));
        inputs.push(format!(
            "fn f() -> u8 {{ {}",
            format!("{spelling} ").repeat(2_000)
        ));
    }
    for statement in [
        "for i in f ( ;",
        "for i in 0..g ( ;",
        "for i in a(b)(c)(d) && e(f) .. g(h)(i) (s: u8 = 0) { continue(s) }",
        "let x = ;",
        "let = 1;",
        "x y",
        "f(,);",
        "match x { 0 => }",
        "S { x: }",
        "if c { } else",
        "loop (s: u8 = ) -> u8 { }",
    ] {
        inputs.push(format!(
            "fn f() -> u8 {{ {} 1 }}",
            format!("{statement} ").repeat(2_000)
        ));
    }
    inputs.push("fn f( {} ".repeat(2_000));
    inputs.push("struct S { x: } enum E { A( } prop P { Q: } const c: = ;".repeat(500));
    assert!(inputs.len() > 300);

    let mut worst = 0.0f64;
    for text in &inputs {
        let mut sources = SourceMap::default();
        let file = sources.add("steps.lc", text.as_str());
        let stats = parse(sources.get(file)).stats;
        assert!(stats.tokens >= 1, "the end-of-file token is always present");
        assert!(
            stats.steps <= STEP_FACTOR * stats.tokens,
            "{} steps on {} tokens: {:?}",
            stats.steps,
            stats.tokens,
            text.chars().take(200).collect::<String>()
        );
        worst = worst.max(stats.steps as f64 / stats.tokens as f64);
    }
    println!("most steps per token: {worst:.3}");
}
