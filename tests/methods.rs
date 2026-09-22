//! Inherent `impl` blocks and `self` (O4), from the elaborator's side: how
//! a method resolves, what a call of one lends or moves, what `Self` and
//! `*self` mean, and what is refused. Where a refusal quotes a rustc code,
//! the Rust of the same shape is handed to rustc, which must refuse it
//! with that code, so that Locus never quotes a code rustc would not give.

use std::path::PathBuf;
use std::process::Command;

use locus::elab::{Elaborated, elaborate};
use locus::erased::{Interpreter, Value, check_module, print_module};
use locus::parser::parse;
use locus::source::SourceMap;
use locus::typed::FnRef;

const FUEL: u64 = 1_000_000;

const PRELUDE: &str = "
#[derive(Clone, Copy, Debug)]
struct Counter {
    count: u8,
}

#[derive(Debug)]
struct Token {
    id: u8,
}

#[derive(Debug)]
enum Slot {
    Empty,
    Full(Token),
}
";

fn elaborated(body: &str) -> Elaborated {
    let text = format!("{PRELUDE}\n{body}\n");
    let mut sources = SourceMap::default();
    let file = sources.add("methods.lc", &text);
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{text}\n{:#?}", parsed.diagnostics);
    elaborate(source, &parsed.program)
}

fn accepted(body: &str) -> Elaborated {
    let result = elaborated(body);
    let messages: Vec<String> = result
        .diagnostics
        .iter()
        .map(|d| format!("{}: {} {:?}", d.code, d.message, d.notes))
        .collect();
    assert!(result.is_success(), "{messages:#?}\n{body}");
    assert_eq!(check_module(result.session.erased()), Ok(()));
    result
}

/// The codes of the diagnostics, and each one's message.
fn rejected(body: &str) -> Vec<(&'static str, String)> {
    let result = elaborated(body);
    assert!(!result.is_success(), "accepted:\n{body}");
    result
        .diagnostics
        .iter()
        .map(|d| (d.code, d.message.clone()))
        .collect()
}

fn call(result: &Elaborated, name: &str, arguments: Vec<Value>) -> String {
    let module = result.session.erased();
    let function = result.function(name).expect("the function exists");
    Interpreter::new(module, FUEL)
        .call(function, arguments)
        .expect("the call returns")
        .debug(module)
}

/// The error codes rustc gives a Rust source, compiled as a library.
fn rustc_codes(name: &str, source: &str) -> Vec<String> {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let source_path = directory.join(format!("methods_{name}.rs"));
    let output_path = directory.join(format!("libmethods_{name}.rlib"));
    std::fs::write(&source_path, source).unwrap();
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let compile = Command::new(rustc)
        .args(["--edition", "2024", "--crate-type", "lib", "-o"])
        .arg(&output_path)
        .arg(&source_path)
        .output()
        .expect("rustc runs");
    assert!(!compile.status.success(), "rustc accepted:\n{source}");
    String::from_utf8_lossy(&compile.stderr)
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("error[")?;
            Some(rest[..rest.find(']')?].to_string())
        })
        .collect()
}

#[test]
fn a_method_resolves_by_the_receiver_s_type_and_is_called_as_rust_writes_it() {
    let result = accepted(
        "
impl Counter {
    fn fresh() -> Self { Self { count: 0 } }
    fn get(&self) -> u8 { self.count }
    fn bump(&mut self) -> () { self.count = self.count.wrapping_add(1); }
    fn grown(mut self) -> Counter { self.count = self.count.wrapping_add(10); self }
}

fn run(n: u8) -> u8 {
    let mut c = Counter::fresh();
    c.bump();
    c.count = c.count.wrapping_add(n);
    Counter::bump(&mut c);
    let grown = c.grown();
    grown.get()
}
",
    );
    assert_eq!(call(&result, "run", vec![Value::u8(5)]), "17");
    // The functions are filed as `Type::name`, and `run` sees them.
    for name in [
        "Counter::fresh",
        "Counter::get",
        "Counter::bump",
        "Counter::grown",
    ] {
        assert!(result.function(name).is_some(), "{name} is not declared");
    }
    let rust = print_module(result.session.erased());
    // A method called by path, `Counter::bump(&mut c)`, is printed as the
    // method call it is, `c.bump()`, like the one written that way.
    assert_eq!(rust.matches("    c.bump();\n").count(), 2, "{rust}");
    assert!(!rust.contains("Counter::bump("), "{rust}");
    for expected in [
        "impl Counter {\n",
        "    pub fn fresh() -> Counter {",
        "    pub fn get(&self) -> u8 {",
        "    pub fn bump(&mut self) -> () {",
        "    pub fn grown(mut self) -> Counter {",
        "    let grown = c.grown();",
        "    grown.get()",
        "let mut c = Counter::fresh();",
    ] {
        assert!(rust.contains(expected), "missing {expected:?} in:\n{rust}");
    }
}

#[test]
fn a_method_of_the_logic_is_a_kernel_function_named_by_its_type() {
    let result = accepted(
        "
impl Counter {
    #[terminates] #[no_panic] #[no_io]
    fn small(&self) -> Prop { prop!(self.count <= 3) }

    #[terminates] #[no_panic] #[no_io]
    fn get(&self) -> u8 { self.count }
}

fn opened(c: Counter, h: @(c.small())) -> @(c.count <= 3) {
    unfold!(Counter::small, h)
}

fn closed(c: Counter, h: @(c.count <= 3)) -> @Counter::small(&c) {
    fold!(Counter::small, h)
}

fn through_get(c: Counter, h: @(c.count <= 3)) -> @(c.get() <= 3) {
    fold!(Counter::get, h)
}
",
    );
    for name in ["Counter::small", "Counter::get"] {
        assert!(
            matches!(result.function(name), Some(FnRef::Math(_))),
            "{name} is not a function of the logic"
        );
    }
    // `small` returns a `Prop` and has no runtime form; `get` runs.
    let rust = print_module(result.session.erased());
    assert!(!rust.contains("fn small"), "{rust}");
    assert!(rust.contains("    pub fn get(&self) -> u8 {"), "{rust}");
}

#[test]
fn self_is_the_type_of_the_block_everywhere_a_type_is_written() {
    let result = accepted(
        "
impl Slot {
    fn empty() -> Self { Self::Empty }
    fn holding(id: u8) -> Self { Self::Full(Token { id }) }
    fn id(&self) -> u8 {
        match *self {
            Self::Full(_) => 1,
            Self::Empty => 0,
        }
    }
    fn take(self) -> u8 {
        match self {
            Self::Full(t) => t.id,
            Self::Empty => 0,
        }
    }
}

fn run() -> u8 {
    let s = Slot::holding(7);
    s.id().wrapping_add(s.take()).wrapping_add(Slot::empty().take())
}
",
    );
    assert_eq!(call(&result, "run", vec![]), "8");
}

#[test]
fn a_self_receiver_moves_the_value_and_a_reference_receiver_does_not() {
    let (codes, _) = rejected(
        "
impl Token {
    fn consume(self) -> u8 { self.id }
    fn id(&self) -> u8 { self.id }
}

fn run() -> u8 {
    let t = Token { id: 1 };
    let a = t.consume();
    a.wrapping_add(t.id())
}
",
    )
    .into_iter()
    .next()
    .unwrap();
    assert_eq!(codes, "L0240");
    let result = accepted(
        "
impl Token {
    fn consume(self) -> u8 { self.id }
    fn id(&self) -> u8 { self.id }
}

fn run() -> u8 {
    let t = Token { id: 1 };
    let a = t.id();
    a.wrapping_add(t.consume())
}
",
    );
    assert_eq!(call(&result, "run", vec![]), "2");
}

#[test]
fn a_pattern_that_moves_out_of_star_self_is_refused_as_rustc_refuses_it() {
    let diagnostics = rejected(
        "
impl Slot {
    fn id(&self) -> u8 {
        match *self {
            Slot::Full(t) => t.id,
            Slot::Empty => 0,
        }
    }
}
",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].0, "L0265");
    assert!(diagnostics[0].1.contains("(E0507)"), "{}", diagnostics[0].1);
    assert_eq!(
        rustc_codes(
            "move_out_of_self",
            "struct Token { id: u8 }\nenum Slot { Empty, Full(Token) }\nimpl Slot {\n    pub fn id(&self) -> u8 {\n        match *self {\n            Slot::Full(t) => t.id,\n            Slot::Empty => 0,\n        }\n    }\n}\n"
        ),
        ["E0507"]
    );
}

/// Each refusal that quotes a rustc code, with the Rust of its shape.
const SHAPES: &[(&str, &str, &str, &str, &str)] = &[
    (
        "assign through &self",
        "impl Counter { fn set(&self, v: u8) -> () { *self = Counter { count: v }; } }",
        "L0266",
        "cannot assign to `*self`, which is behind a `&` reference (E0594)",
        "pub struct Counter { count: u8 }\nimpl Counter { pub fn set(&self, v: u8) { *self = Counter { count: v }; } }\n",
    ),
    (
        "assign a field through &self",
        "impl Counter { fn set(&self, v: u8) -> () { self.count = v; } }",
        "L0266",
        "cannot assign to `self.count`, which is behind a `&` reference (E0594)",
        "pub struct Counter { count: u8 }\nimpl Counter { pub fn set(&self, v: u8) { self.count = v; } }\n",
    ),
    (
        "&mut self on an immutable local",
        "impl Counter { fn bump(&mut self) -> () { self.count = 1; } }\nfn run() -> u8 { let c = Counter { count: 0 }; c.bump(); c.count }",
        "L0262",
        "cannot borrow `c` as mutable, as it is not declared as mutable (E0596)",
        "pub struct Counter { count: u8 }\nimpl Counter { pub fn bump(&mut self) { self.count = 1; } }\npub fn run() -> u8 { let c = Counter { count: 0 }; c.bump(); c.count }\n",
    ),
    (
        "*self on self by value",
        "impl Counter { fn get(self) -> u8 { (*self).count } }",
        "L0266",
        "type `Counter` cannot be dereferenced (E0614)",
        "pub struct Counter { count: u8 }\nimpl Counter { pub fn get(self) -> u8 { (*self).count } }\n",
    ),
    (
        "a method the type does not have",
        "fn run(c: Counter) -> u8 { c.size() }",
        "L0207",
        "no method named `size` found for `Counter` (E0599)",
        "pub struct Counter { count: u8 }\npub fn run(c: Counter) -> u8 { c.size() }\n",
    ),
];

#[test]
fn every_refusal_that_quotes_rustc_is_rustc_s_too() {
    for (name, body, code, message, rust) in SHAPES {
        let diagnostics = rejected(body);
        assert!(
            diagnostics
                .iter()
                .any(|(found, text)| found == code && text.contains(message)),
            "{name}: expected {code} `{message}`, found {diagnostics:#?}"
        );
        let expected = &message[message.rfind('(').unwrap() + 1..message.len() - 1];
        let codes = rustc_codes(&name.replace(' ', "_").replace(['&', '*'], "x"), rust);
        assert_eq!(codes, [expected], "{name}");
    }
}

#[test]
fn the_export_boundary_applies_to_methods() {
    let diagnostics = rejected(
        "
impl Counter {
    pub fn small(&self, h: @(self.count <= 3)) -> u8 { self.count }
}
",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].0, "L0244");
    assert!(
        diagnostics[0]
            .1
            .starts_with("`Counter::small` takes evidence and cannot be `pub`"),
        "{}",
        diagnostics[0].1
    );
    // The receiver is not evidence: a `pub` method on a validated type is
    // fine, and `pub(crate)` takes evidence as any function may.
    accepted(
        "
impl Counter {
    pub fn get(&self) -> u8 { self.count }
    pub(crate) fn small(&self, h: @(self.count <= 3)) -> u8 { self.count }
}
",
    );
}

#[test]
fn an_impl_block_of_an_unknown_type_is_reported_once_and_its_functions_are_skipped() {
    let diagnostics = rejected(
        "
impl Missing {
    fn get(&self) -> u8 { self.count }
    fn other(&self) -> u8 { self.get() }
}

fn run(c: Counter) -> u8 { c.get() }
",
    );
    assert_eq!(diagnostics.len(), 2, "{diagnostics:#?}");
    assert_eq!(diagnostics[0], ("L0200", "unknown type `Missing`".into()));
    assert_eq!(diagnostics[1].0, "L0207");
    assert!(
        diagnostics[1]
            .1
            .contains("no method named `get` found for `Counter`"),
        "{}",
        diagnostics[1].1
    );
}

#[test]
fn a_method_and_a_free_function_may_share_a_name_and_a_variant_may_not() {
    let result = accepted(
        "
impl Counter {
    fn get(&self) -> u8 { self.count }
}

fn get(c: Counter) -> u8 { c.get().wrapping_add(1) }

fn run() -> u8 { get(Counter { count: 1 }) }
",
    );
    assert_eq!(call(&result, "run", vec![]), "2");
    let diagnostics = rejected(
        "
impl Slot {
    fn Empty(&self) -> u8 { 0 }
}
",
    );
    assert_eq!(
        diagnostics,
        [("L0202", "`Slot::Empty` is declared twice".to_string())]
    );
}

#[test]
fn an_impl_block_is_for_the_type_even_when_a_function_shares_its_name() {
    // Types and values are two namespaces (E9): the block is for the
    // struct `Counter`, whatever a function `Counter` declared earlier is.
    let result = accepted(
        "
#[terminates] #[no_panic] #[no_io]
fn Counter(n: u8) -> Prop { prop!(n <= 3) }

impl Counter {
    fn get(&self) -> u8 { self.count }
}

fn run(n: u8) -> u8 { let c = Counter { count: n }; c.get() }

fn claimed(c: Counter, h: @Counter(c.count)) -> @(c.count <= 3) { unfold!(Counter, h) }
",
    );
    assert_eq!(call(&result, "run", vec![Value::u8(2)]), "2");
}

#[test]
fn a_method_called_as_a_free_function_is_unknown_and_the_note_names_its_type() {
    let result = elaborated(
        "
impl Counter {
    fn get(&self) -> u8 { self.count }
}

fn run(c: Counter) -> u8 { get(c) }
",
    );
    assert_eq!(result.diagnostics.len(), 1, "{:#?}", result.diagnostics);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.code, "L0204");
    assert_eq!(diagnostic.message, "unknown function `get`");
    assert!(
        diagnostic.notes.iter().any(|note| note
            .contains("`get` is declared in `impl Counter`: call it as `Counter::get(..)`")),
        "{:#?}",
        diagnostic.notes
    );
}

#[test]
fn the_result_of_a_mut_self_method_speaks_of_self_at_return() {
    let result = accepted(
        "
impl Counter {
    #[no_io]
    fn reset(&mut self) -> @(self.count == 0) {
        *self = Counter { count: 0 };
        prove!(self.count == 0)
    }

    #[no_io]
    fn kept(&mut self) -> @(self.count == old!(self).count) {
        prove!(self.count == old!(self).count)
    }
}

fn run() -> u8 {
    let mut c = Counter { count: 9 };
    let zero = c.reset();
    let _ = zero;
    let _ = c.kept();
    c.count
}
",
    );
    assert_eq!(call(&result, "run", vec![]), "0");
    let rust = print_module(result.session.erased());
    assert!(rust.contains("*self = Counter { count: 0_u8 };"), "{rust}");
}
