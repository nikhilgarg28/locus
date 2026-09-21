use locus::ast::{
    BinaryOp, Block, DeclarationKind, Expr, ExprKind, Form, FunctionMode, IntegerLiteral,
    IntegerSuffix, PatternKind, StatementKind, TypeKind,
};
use locus::diagnostic::Applicability;
use locus::kernel::Natural;
use locus::lexer::{Lexed, Literal, TokenKind as K, lex};
use locus::parser::{Parsed, parse};
use locus::source::{FileId, SourceMap, Span};

fn parse_text(text: &str) -> Parsed {
    let mut sources = SourceMap::default();
    let file = sources.add("test.lc", text);
    parse(sources.get(file))
}

fn expression(text: &str) -> Expr {
    let parsed = parse_text(&format!("fn example() -> u8 {{ {text} }}"));
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let DeclarationKind::Function { mut body, .. } =
        parsed.program.declarations.into_iter().next().unwrap().kind
    else {
        panic!()
    };
    *body.tail.take().unwrap()
}

/// The formula of `prop!(text)`, where `=>` and the quantifiers are read.
fn formula(text: &str) -> Expr {
    let ExprKind::Form {
        form: Form::Prop,
        mut arguments,
        ..
    } = expression(&format!("prop!({text})")).kind
    else {
        panic!("not a prop! form")
    };
    arguments.pop().unwrap()
}

fn binary(expression: &Expr, expected: BinaryOp) -> (&Expr, &Expr) {
    let ExprKind::Binary {
        operator,
        left,
        right,
        ..
    } = &expression.kind
    else {
        panic!("expected binary, got {expression:?}")
    };
    assert_eq!(*operator, expected);
    (left, right)
}

#[test]
fn source_locations_use_bytes_without_splitting_unicode() {
    let mut sources = SourceMap::default();
    let file = sources.add("unicode.lc", "aé\r\nb\n");
    let source = sources.get(file);
    assert_eq!(source.line_column(3), Some((1, 3)));
    assert_eq!(source.line_column(5), Some((2, 1)));
    assert_eq!(source.line_column(7), Some((3, 1)));
    assert_eq!(source.line_column(2), None);
    assert_eq!(source.slice(Span::new(file, 1, 3)), Some("é"));
    assert_eq!(source.slice(Span::new(FileId(99), 1, 3)), None);
}

#[test]
fn lexer_preserves_large_literals_and_distinguishes_keywords() {
    let text = "fn fn_name forall_ _ 1_000 9999999999999999999999999999999999999999 == => -> != <= >= && || @ # const def Prop prop math :: .. match in exists";
    let mut sources = SourceMap::default();
    let file = sources.add("test.lc", text);
    let source = sources.get(file);
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let kinds: Vec<_> = lexed.tokens.iter().map(|token| token.kind).collect();
    assert_eq!(
        kinds,
        [
            K::Fn,
            K::Name,
            K::Name,
            K::Underscore,
            K::Integer,
            K::Integer,
            K::EqualEqual,
            K::Implies,
            K::Arrow,
            K::BangEqual,
            K::LessEqual,
            K::GreaterEqual,
            K::AndAnd,
            K::OrOr,
            K::At,
            K::Hash,
            K::Const,
            K::Name,
            K::Name,
            K::Name,
            K::Name,
            K::PathSep,
            K::DotDot,
            K::Match,
            K::In,
            K::Name,
            K::Eof
        ]
    );
    assert_eq!(
        source.slice(lexed.tokens[5].span).unwrap(),
        "9999999999999999999999999999999999999999"
    );
    assert_eq!(lexed.tokens.last().unwrap().span.start, text.len());
}

#[test]
fn nested_comments_and_line_comments_are_skipped() {
    let mut sources = SourceMap::default();
    let file = sources.add("test.lc", "/* outer /* inner */ end */ fn // tail\n x");
    let lexed = lex(sources.get(file));
    assert!(lexed.diagnostics.is_empty());
    assert_eq!(
        lexed
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [K::Fn, K::Name, K::Eof]
    );
}

#[test]
fn lexical_errors_are_reported_once_with_valid_spans() {
    for (text, code) in [
        ("/* missing", "L0002"),
        ("12u9", "L0003"),
        ("0b102", "L0003"),
        ("café", "L0004"),
        ("r#type", "L0005"),
        ("\"a string", "L0006"),
        ("\"a \\q string\"", "L0007"),
        ("💡", "L0001"),
        ("\\", "L0001"),
        ("`", "L0001"),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let lexed = lex(source);
        assert_eq!(
            lexed.diagnostics.len(),
            1,
            "{text}: {:?}",
            lexed.diagnostics
        );
        assert_eq!(lexed.diagnostics[0].code, code);
        for token in lexed.tokens {
            assert!(source.slice(token.span).is_some());
        }
    }
}

#[test]
fn all_acceptance_examples_parse() {
    for example in [
        include_str!("../examples/increment.lc"),
        include_str!("../examples/preserve.lc"),
        include_str!("../examples/proofs.lc"),
        include_str!("../examples/propositions.lc"),
        include_str!("../examples/lock.lc"),
    ] {
        let parsed = parse_text(example);
        assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    }
}

#[test]
fn equality_conjunction_and_implication_have_distinct_precedence() {
    let expr = formula("a.wrapping_add(1) == b && ready => done");
    let (condition, _) = binary(&expr, BinaryOp::Implies);
    let (comparison, _) = binary(condition, BinaryOp::And);
    let (left, _) = binary(comparison, BinaryOp::Equal);
    assert!(matches!(left.kind, ExprKind::Call { .. }));
}

#[test]
fn implication_is_right_associative_and_addition_is_retired() {
    let expr = formula("a => b => c");
    let (_, right) = binary(&expr, BinaryOp::Implies);
    binary(right, BinaryOp::Implies);
    let parsed = parse_text("fn f(a: u8) -> u8 { a + 1 }");
    let error = parsed
        .diagnostics
        .iter()
        .find(|d| d.code == "L0112")
        .unwrap();
    assert!(error.notes.iter().any(|note| note.contains("wrapping_add")));
}

#[test]
fn comparisons_reject_chaining_but_allow_explicit_grouping() {
    for text in ["a == b == c", "a < b <= c", "a != b > c"] {
        let parsed = parse_text(&format!("fn f() -> bool {{ {text} }}"));
        assert!(parsed.diagnostics.iter().any(|error| error.code == "L0103"));
    }
    expression("(a == b) == c");
}

#[test]
fn grouping_unit_and_singleton_tuple_stay_distinct() {
    assert!(matches!(expression("()").kind, ExprKind::Unit));
    assert!(matches!(expression("(n)").kind, ExprKind::Group(_)));
    assert!(matches!(expression("(n,)").kind, ExprKind::Tuple(elements) if elements.len() == 1));
    assert!(matches!(expression("(n, m,)").kind, ExprKind::Tuple(elements) if elements.len() == 2));
}

#[test]
fn a_hole_is_its_own_node_and_proof_blocks_are_retired() {
    assert!(matches!(expression("_").kind, ExprKind::Hole));
    let parsed = parse_text("fn f() -> @(true) { @{ reflexivity; } }");
    let error = parsed
        .diagnostics
        .iter()
        .find(|d| d.code == "L0110")
        .unwrap();
    assert!(error.notes.iter().any(|note| note.contains("retired")));
}

#[test]
fn dependent_results_and_destructuring_keep_their_binders() {
    let parsed =
        parse_text("fn f(n: u8) -> (out: u8, @(out == n)) { let (value, _) = (n, _); (value, _) }");
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Function { result, body, .. } = &parsed.program.declarations[0].kind
    else {
        panic!()
    };
    let TypeKind::Tuple(fields) = &result.kind else {
        panic!()
    };
    assert_eq!(fields[0].name.as_ref().unwrap().text, "out");
    assert!(matches!(fields[1].ty.kind, TypeKind::Proof(_)));
    let StatementKind::Let { pattern, .. } = &body.statements[0].kind else {
        panic!()
    };
    let PatternKind::Tuple(elements) = &pattern.kind else {
        panic!()
    };
    assert!(matches!(elements[1].kind, PatternKind::Wildcard));
}

#[test]
fn calls_and_members_bind_tighter_than_prefix_not() {
    let ExprKind::Not(value) = expression("!x.test(1)").kind else {
        panic!()
    };
    let ExprKind::Call { callee, arguments } = value.kind else {
        panic!()
    };
    assert_eq!(arguments.len(), 1);
    assert!(matches!(callee.kind, ExprKind::Member { .. }));
}

#[test]
fn missing_semicolon_has_an_applicable_fix_and_recovers_next_binding() {
    let text = "fn f() -> u8 { let x = 1\n let y = 2; y }";
    let parsed = parse_text(text);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let error = &parsed.diagnostics[0];
    assert_eq!(error.code, "L0102");
    let fix = &error.suggestions[0];
    assert_eq!(fix.applicability, Applicability::MachineApplicable);
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    assert!(parse_text(&fixed).is_success(), "{fixed}");
    let DeclarationKind::Function { body, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert!(matches!(body.statements[1].kind, StatementKind::Let { .. }));
}

#[test]
fn singleton_named_type_fix_is_valid() {
    let text = "fn f() -> (out: u8) { (1,) }";
    let parsed = parse_text(text);
    let error = parsed
        .diagnostics
        .iter()
        .find(|error| error.code == "L0106")
        .unwrap();
    let fix = &error.suggestions[0];
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    assert!(parse_text(&fixed).is_success());
}

#[test]
fn recovery_preserves_later_declarations_and_independent_errors() {
    let parsed =
        parse_text("fn broken( {} fn good() -> u8 { 1 } fn bad() -> u8 { let x = ; let y = ; 2 }");
    assert!(!parsed.is_success());
    assert_eq!(parsed.program.declarations.len(), 2, "{:?}", parsed);
    assert_eq!(parsed.diagnostics.len(), 3, "{:?}", parsed.diagnostics);
}

#[test]
fn unmatched_delimiter_points_back_to_its_opening() {
    let parsed = parse_text("fn f() -> u8 { (1");
    let error = parsed
        .diagnostics
        .iter()
        .find(|error| error.code == "L0101")
        .unwrap();
    assert!(
        error
            .labels
            .iter()
            .any(|label| label.message == "delimiter opened here")
    );
}

#[test]
fn attributes_are_reserved_without_becoming_proofs() {
    for prefix in ["#[test]", "#![allow(unused)]"] {
        let parsed = parse_text(&format!("{prefix} fn f() -> u8 {{ 1 }}"));
        assert_eq!(parsed.diagnostics[0].code, "L0105");
        assert_eq!(parsed.program.declarations.len(), 1);
    }
}

#[test]
fn if_requires_else_and_else_if_is_supported() {
    let parsed = parse_text("fn f() -> u8 { if true { 1 } }");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|error| error.message.contains("requires an `else`"))
    );
    expression("if a { 1 } else if b { 2 } else { 3 }");
}

#[test]
fn syntax_success_does_not_claim_proof_validity() {
    assert!(parse_text("fn impossible() -> @(false) { _ }").is_success());
}

#[test]
fn diagnostic_rendering_contains_source_location_and_help() {
    let mut sources = SourceMap::default();
    let file = sources.add("example.lc", "fn f() -> u8 {\n    let n = 1\n    n\n}\n");
    let parsed = parse(sources.get(file));
    let rendered = parsed.diagnostics[0].render(&sources, false);
    assert!(rendered.contains("error[L0102]"), "{rendered}");
    assert!(rendered.contains("example.lc:2:"), "{rendered}");
    assert!(rendered.contains("let n = 1"), "{rendered}");
    assert!(rendered.contains("insert `;` here"), "{rendered}");
    assert!(!rendered.contains('\u{1b}'));
}

#[test]
fn deeply_nested_input_reports_a_limit_instead_of_overflowing_the_stack() {
    // Half the default thread stack, so the limit keeps a margin.
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let limit_reported = |text: String, form: &str| {
                assert!(
                    parse_text(&text)
                        .diagnostics
                        .iter()
                        .any(|error| error.code == "L0108"),
                    "{form}"
                );
            };
            for (open, close) in [
                ("(", ")"),
                ("(1, ", ")"),
                ("[", "]"),
                ("{", "}"),
                ("{ let x = ", "; 1 }"),
                ("!", ""),
                ("prop!(p => ", ")"),
                ("prove!(", ")"),
                ("prop!(", ")"),
                ("f(", ")"),
                ("x.g(", ")"),
                ("if c { 1 } else { ", " }"),
                ("if c { 1 } else ", ""),
                ("if ", " { 1 } else { 1 }"),
                ("match x { _ => ", " }"),
                ("match ", " { _ => 1 }"),
                ("S { x: ", " }"),
                ("for i in 0..n (s: u8 = ", ") { continue(s) }"),
                ("for i in ", "..n () { continue() }"),
                ("loop () -> u8 { break ", " }"),
                ("loop (s: u8 = ", ") -> u8 { break s }"),
                ("break ", ""),
                ("continue(", ")"),
                ("prop!(forall (n: u8) { ", " })"),
                ("prop!(exists (n: u8) { ", " })"),
            ] {
                limit_reported(
                    format!(
                        "fn f() -> u8 {{ {}1{} }}",
                        open.repeat(512),
                        close.repeat(512)
                    ),
                    open,
                );
            }
            for (open, close) in [
                ("(", ")"),
                ("(x: ", ",)"),
                ("(u8, ", ")"),
                ("fn(", ") -> u8"),
                ("fn() -> ", ""),
                ("math fn(x: ", ") -> u8"),
                ("@(forall (h: ", ") { true })"),
            ] {
                limit_reported(
                    format!(
                        "fn f() -> {}u8{} {{ 1 }}",
                        open.repeat(512),
                        close.repeat(512)
                    ),
                    open,
                );
            }
            for (open, close) in [
                ("(", ")"),
                ("(_, ", ")"),
                ("E::V(", ")"),
                ("S { x: ", " }"),
                ("S { ", " }"),
            ] {
                let (open, close) = (open.repeat(512), close.repeat(512));
                limit_reported(
                    format!("fn f() -> u8 {{ let {open}x{close} = 1; 1 }}"),
                    &open,
                );
                limit_reported(
                    format!("fn f() -> u8 {{ match y {{ {open}x{close} => 1 }} }}"),
                    &open,
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn malformed_inputs_terminate_and_keep_valid_diagnostic_spans() {
    let alphabet = [
        "fn ", "def ", "const ", "let ", "@", "_", "[", "]", "#", "||", "(", ")", "{", "}", ";",
        "prove!(", "prop!(", "!", "forall ", "exists ", "=>", "=", "n", "0", "💡", "é", "\n", "/*",
        "*/", "math ", "prop ", "struct ", "enum ", "match ", "loop ", "for ", "in ", "..", "::",
        "break ", "continue", ",", ":", ".", "->",
    ];
    let mut seed = 17u64;
    for length in 0..256 {
        let mut text = String::new();
        for _ in 0..length % 64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(alphabet[(seed >> 32) as usize % alphabet.len()]);
        }
        let mut sources = SourceMap::default();
        let file = sources.add("fuzz.lc", &text);
        let source = sources.get(file);
        let parsed = parse(source);
        for error in parsed.diagnostics {
            for label in &error.labels {
                assert!(source.slice(label.span).is_some(), "{text:?}: {label:?}");
            }
            let _ = error.render(&sources, false);
        }
    }
}

#[test]
fn constants_and_math_functions_state_propositions() {
    let parsed = parse_text(
        "const reflexive: Prop = prop!(forall (n: u8) { n == n });
         math fn same(x: u8, y: u8) -> Prop { prop!(x == y) }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Constant { name, ty, value } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert_eq!(name.text, "reflexive");
    assert!(matches!(&ty.kind, TypeKind::Named(name) if name.text == "Prop"));
    assert!(
        matches!(&value.kind, ExprKind::Form { form: Form::Prop, arguments, .. } if matches!(arguments[0].kind, ExprKind::Forall { .. }))
    );
    let DeclarationKind::Function { result, body, .. } = &parsed.program.declarations[1].kind
    else {
        panic!()
    };
    assert!(matches!(&result.kind, TypeKind::Named(name) if name.text == "Prop"));
    assert!(matches!(
        body.tail.as_ref().unwrap().kind,
        ExprKind::Form {
            form: Form::Prop,
            ..
        }
    ));
    assert!(!parse_text("prop Same(x: u8) = x == x;").is_success());
}

#[test]
fn math_and_prop_are_keywords_only_where_a_declaration_can_begin() {
    let parsed = parse_text(
        "math fn prop(math: u8) -> u8 { let prop = math; prop }
         fn math(prop: u8) -> u8 { prop }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
}

#[test]
fn def_is_reported_once_with_a_fix_and_still_parses() {
    let text = "def same(x: u8, y: u8) -> Prop { prop!(x == y) }";
    let parsed = parse_text(text);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(parsed.diagnostics[0].code, "L0113");
    assert_eq!(parsed.program.declarations.len(), 1);
    let fix = &parsed.diagnostics[0].suggestions[0];
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    assert!(parse_text(&fixed).is_success(), "{fixed}");
    // Elsewhere `def` is a name, as `forall` and `exists` are.
    assert!(
        parse_text("fn def(forall: u8, exists: u8) -> u8 { let def = forall; def }").is_success()
    );
}

#[test]
fn proof_types_accept_named_inline_and_called_propositions_with_precise_spans() {
    for (target, spelling) in [
        ("claim", "@claim"),
        ("(n == n)", "@ (n == n)"),
        ("same(n, n)", "@same(n, n)"),
    ] {
        let mut sources = SourceMap::default();
        let text = format!("fn f(n: u8) -> {spelling} {{ _ }}");
        let file = sources.add("proof-type.lc", text);
        let source = sources.get(file);
        let parsed = parse(source);
        assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
        let DeclarationKind::Function { result, .. } = &parsed.program.declarations[0].kind else {
            panic!()
        };
        let TypeKind::Proof(proposition) = &result.kind else {
            panic!()
        };
        assert_eq!(source.slice(result.span), Some(spelling));
        assert_eq!(source.slice(proposition.span), Some(target));
        match target {
            "claim" => assert!(matches!(proposition.kind, ExprKind::Name(_))),
            "(n == n)" => assert!(matches!(proposition.kind, ExprKind::Group(_))),
            _ => assert!(matches!(proposition.kind, ExprKind::Call { .. })),
        }
    }
}

#[test]
fn proof_holes_and_wildcard_patterns_are_different_nodes() {
    let parsed = parse_text("fn f() -> () { let (_, evidence) = (1, _); () }");
    assert!(parsed.is_success());
    let DeclarationKind::Function { body, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    let StatementKind::Let { pattern, value, .. } = &body.statements[0].kind else {
        panic!()
    };
    let PatternKind::Tuple(patterns) = &pattern.kind else {
        panic!()
    };
    let ExprKind::Tuple(values) = &value.kind else {
        panic!()
    };
    assert!(matches!(patterns[0].kind, PatternKind::Wildcard));
    assert!(matches!(values[1].kind, ExprKind::Hole));
}

/// Every fix in the diagnostics applied to `text`, last first so that the
/// earlier spans stay right.
fn fixed(text: &str, parsed: &Parsed) -> String {
    let mut fixed = text.to_owned();
    let mut fixes: Vec<_> = parsed
        .diagnostics
        .iter()
        .flat_map(|diagnostic| diagnostic.suggestions.iter())
        .collect();
    fixes.sort_by_key(|fix| std::cmp::Reverse(fix.span.start));
    for fix in fixes {
        fixed.replace_range(fix.span.range(), &fix.replacement);
    }
    fixed
}

#[test]
fn retired_brackets_are_reported_with_a_fix_that_parses() {
    // A proposition literal, a proof type, one spanning lines, and one
    // nested in a formula: each is L0117 once, with its own fix, and the
    // file goes on being parsed so that every one is reported.
    let text = "math fn same(x: u8, y: u8) -> Prop { [x == y] }
fn f(n: u8) -> (out: u8, @[out == n]) {
    let claim: Prop = [
        forall (k: u8) { k == k => [k <= 255] }
    ];
    let h: @[n == n] = _;
    (n, _)
}
prop P(n: u8) { Small: @[n < 10] }
const c: Prop = [true];";
    let parsed = parse_text(text);
    assert_eq!(
        parsed.program.declarations.len(),
        4,
        "{:#?}",
        parsed.diagnostics
    );
    let codes: Vec<_> = parsed.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["L0117"; 7], "{:#?}", parsed.diagnostics);
    // One fix per delimiter, so that nested brackets can all be fixed at
    // once.
    let mut fixes = Vec::new();
    for diagnostic in &parsed.diagnostics {
        assert!(diagnostic.message.contains("brackets are for arrays"));
        assert_eq!(diagnostic.suggestions.len(), 2);
        let mut pair = Vec::new();
        for fix in &diagnostic.suggestions {
            assert_eq!(fix.applicability, Applicability::MaybeIncorrect);
            pair.push((&text[fix.span.range()], fix.replacement.as_str()));
        }
        fixes.push((pair[0], pair[1], &text[diagnostic.labels[0].span.range()]));
    }
    assert_eq!(
        fixes,
        [
            (("[", "prop!("), ("]", ")"), "[x == y]"),
            (("@[", "@("), ("]", ")"), "@[out == n]"),
            (
                ("[", "prop!("),
                ("]", ")"),
                "[\n        forall (k: u8) { k == k => [k <= 255] }\n    ]"
            ),
            (("[", "prop!("), ("]", ")"), "[k <= 255]"),
            (("@[", "@("), ("]", ")"), "@[n == n]"),
            (("@[", "@("), ("]", ")"), "@[n < 10]"),
            (("[", "prop!("), ("]", ")"), "[true]"),
        ]
    );
    let fixed = fixed(text, &parsed);
    let parsed = parse_text(&fixed);
    assert!(parsed.is_success(), "{fixed}\n{:#?}", parsed.diagnostics);
    assert!(fixed.contains("prop!(\n        forall (k: u8) { k == k => prop!(k <= 255) }\n    )"));
    // The old brackets read as the new forms: the same declarations, with
    // the same number of `prop!` nodes among them.
    let count =
        |program: &locus::ast::Program| format!("{program:?}").matches("Form { form: Prop").count();
    assert_eq!(parsed.program.declarations.len(), 4);
    assert_eq!(count(&parse_text(text).program), count(&parsed.program));
    assert_eq!(count(&parsed.program), 4);
}

#[test]
fn brackets_are_arrays_which_are_not_in_locus_yet() {
    for text in ["[]", "[n,]", "[n, m]", "[n; 3]"] {
        let parsed = parse_text(&format!("fn f() -> u8 {{ {text} }}"));
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0116")
            .unwrap_or_else(|| panic!("{text}: {:?}", parsed.diagnostics));
        assert_eq!(error.message, "arrays are not in Locus yet");
    }
    for text in [
        "fn f(xs: [bool; 1]) -> () { () }",
        "fn f(ys: [u8]) -> () { () }",
    ] {
        let parsed = parse_text(text);
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0116")
            .unwrap_or_else(|| panic!("{text}: {:?}", parsed.diagnostics));
        assert_eq!(error.message, "array and slice types are not in Locus yet");
    }
    assert!(parse_text("fn f(h: @(true)) -> () { () }").is_success());
}

#[test]
fn a_prop_form_is_a_proposition_with_or_without_an_annotation() {
    let parsed = parse_text(
        "fn f(n: u8) -> () {
            let claim: Prop = prop!(n > 0);
            let inferred = prop!(n > 0);
            ()
        }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Function { body, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    for statement in &body.statements {
        let StatementKind::Let { value, .. } = &statement.kind else {
            panic!()
        };
        assert!(matches!(
            value.kind,
            ExprKind::Form {
                form: Form::Prop,
                ..
            }
        ));
    }
}

#[test]
fn proposition_operations_have_boolean_style_precedence_and_right_associative_implication() {
    let expr = formula("!p && q || r && s => t => u");
    let (left, right) = binary(&expr, BinaryOp::Implies);
    let (first, second) = binary(left, BinaryOp::Or);
    let (negated, _) = binary(first, BinaryOp::And);
    assert!(matches!(negated.kind, ExprKind::Not(_)));
    binary(second, BinaryOp::And);
    binary(right, BinaryOp::Implies);
    let expr = expression("p || q || r");
    let (left, _) = binary(&expr, BinaryOp::Or);
    binary(left, BinaryOp::Or);
}

#[test]
fn at_is_not_a_bare_proof_hole_or_a_proof_type_in_expression_position() {
    for text in ["@", "@claim", "@[n == n]", "@(n == n)"] {
        let parsed = parse_text(&format!("fn f(n: u8) -> @(n == n) {{ {text} }}"));
        assert!(
            parsed.diagnostics.iter().any(|d| d.code == "L0110"),
            "{text}: {parsed:?}"
        );
    }
    let parsed = parse_text("fn f() -> @ { _ }");
    assert!(!parsed.is_success());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("proof type needs a proposition"))
    );
}

#[test]
fn old_hash_proof_syntax_reports_migration_help() {
    for text in [
        "fn f() -> #(true) { _ }",
        "fn f() -> @(true) { # }",
        "fn f() -> @(true) { #(true) }",
        "fn f() -> @(true) { #{ reflexivity; } }",
    ] {
        let parsed = parse_text(text);
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0111")
            .unwrap();
        assert!(error.message.contains("no longer proof syntax"));
    }
    for text in ["fn f() -> #[true] { _ }", "fn f() -> @(true) { #[true] }"] {
        assert!(
            parse_text(text)
                .diagnostics
                .iter()
                .any(|d| d.code == "L0105")
        );
    }
}

#[test]
fn bracket_errors_report_the_opening_and_preserve_following_declarations() {
    let text = "fn f() -> @[n == n) { _ } const good: Prop = prop!(true);";
    let parsed = parse_text(text);
    let error = parsed
        .diagnostics
        .iter()
        .find(|d| d.code == "L0101")
        .unwrap();
    assert!(
        error
            .labels
            .iter()
            .any(|label| &text[label.span.range()] == "[")
    );
    assert!(matches!(
        parsed.program.declarations.last().unwrap().kind,
        DeclarationKind::Constant { .. }
    ));
    for text in ["[n,,m]", "[n;]", "[n m]", "[n; 3,]"] {
        assert!(!parse_text(&format!("fn f() -> u8 {{ {text} }}")).is_success());
    }
}

#[test]
fn missing_constant_semicolon_fix_and_recovery_work() {
    let text = "const claim: Prop = prop!(true)\nfn good() -> u8 { 1 }";
    let parsed = parse_text(text);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(parsed.program.declarations.len(), 1);
    let fix = &parsed.diagnostics[0].suggestions[0];
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    assert!(parse_text(&fixed).is_success());
}

#[test]
fn a_missing_brace_does_not_consume_the_next_declaration() {
    for next in [
        "fn good() -> u8 { 1 }",
        "math fn good() -> u8 { 1 }",
        "struct Good { x: u8 }",
        "enum Good { A }",
        "prop Good { Trivial }",
    ] {
        let parsed = parse_text(&format!("fn broken() -> u8 {{ let x = 1;\n{next}"));
        assert!(!parsed.is_success());
        assert_eq!(parsed.program.declarations.len(), 1, "{next}: {parsed:?}");
    }
}

#[test]
fn syntax_parser_does_not_pretend_to_enforce_prop_or_hole_types() {
    // These will be semantic errors once typing exists. A syntax-only parse must
    // not be presented as checking the bool/Prop boundary or a proof obligation.
    for text in [
        "fn f(n: u8) -> Prop { n > 0 }",
        "fn f() -> u8 { _ }",
        "fn f(n: u8) -> () { let claim: Prop = n > 0; () }",
        "fn f(n: u8) -> @(n > 0) { _ }",
    ] {
        assert!(parse_text(text).is_success(), "{text}");
    }
}

#[test]
fn fn_and_math_fn_preserve_their_modes() {
    let parsed = parse_text(
        "math fn same(x: u8, y: u8) -> Prop { prop!(x == y) }
         math fn keep(p: Prop, h: @p) -> @p { h }
         fn self_equal(n: u8) -> @(same(n, n)) { _ }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let modes: Vec<_> = parsed
        .program
        .declarations
        .iter()
        .map(|declaration| {
            let DeclarationKind::Function { mode, .. } = declaration.kind else {
                panic!()
            };
            mode
        })
        .collect();
    assert_eq!(
        modes,
        [
            FunctionMode::Math,
            FunctionMode::Math,
            FunctionMode::Runtime
        ]
    );
}

#[test]
fn recovery_keeps_math_functions_after_a_broken_function() {
    let parsed = parse_text(
        "fn broken() -> u8 { 1
         math fn good() -> Prop { prop!(true) }
         const claim: Prop = prop!(true);",
    );
    assert!(!parsed.is_success());
    assert_eq!(parsed.program.declarations.len(), 2, "{parsed:?}");
    assert!(matches!(
        parsed.program.declarations[0].kind,
        DeclarationKind::Function {
            mode: FunctionMode::Math,
            ..
        }
    ));
    assert!(matches!(
        parsed.program.declarations[1].kind,
        DeclarationKind::Constant { .. }
    ));
}

#[test]
fn direct_proposition_call_parses_in_a_dependent_result_tuple() {
    let parsed = parse_text(include_str!("../examples/proofs.lc"));
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let declaration = parsed.program.declarations.iter().find(|declaration| {
        matches!(&declaration.kind, DeclarationKind::Function { name, .. } if name.text == "increment")
    }).expect("the direct-call example must remain present");
    let DeclarationKind::Function { result, .. } = &declaration.kind else {
        panic!()
    };
    let TypeKind::Tuple(fields) = &result.kind else {
        panic!()
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.as_ref().unwrap().text, "out");
    let TypeKind::Proof(proposition) = &fields[1].ty.kind else {
        panic!()
    };
    // The proof target is the call itself, with no surrounding bracket node.
    let ExprKind::Call { callee, arguments } = &proposition.kind else {
        panic!()
    };
    assert!(matches!(&callee.kind, ExprKind::Name(name) if name.text == "same"));
    assert_eq!(arguments.len(), 2);
    assert!(matches!(&arguments[0].kind, ExprKind::Name(name) if name.text == "out"));
    assert!(matches!(&arguments[1].kind, ExprKind::Call { .. }));
}

fn body(text: &str) -> Block {
    let parsed = parse_text(text);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let DeclarationKind::Function { body, .. } =
        parsed.program.declarations.into_iter().last().unwrap().kind
    else {
        panic!()
    };
    body
}

#[test]
fn struct_and_enum_declarations_keep_fields_and_payloads() {
    let parsed = parse_text(
        "struct Lock { failures: u8, open: bool, }
         enum Event { Wrong, Code(u8), Pair(first: u8, second: u8) }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Struct { name, fields } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert_eq!(name.text, "Lock");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].name.text, "open");
    let DeclarationKind::Enum { variants, .. } = &parsed.program.declarations[1].kind else {
        panic!()
    };
    let shapes: Vec<_> = variants
        .iter()
        .map(|variant| (variant.name.text.as_str(), variant.fields.len()))
        .collect();
    assert_eq!(shapes, [("Wrong", 0), ("Code", 1), ("Pair", 2)]);
    assert_eq!(variants[2].fields[1].name.as_ref().unwrap().text, "second");
    assert!(!parse_text("struct Pair { u8, u8 }").is_success());
}

#[test]
fn prop_declarations_keep_parameters_payloads_and_targets() {
    let mut sources = SourceMap::default();
    let file = sources.add(
        "prop.lc",
        "prop Even(n: u8) {
            Zero: @Even(0),
            Step(m: u8, smaller: @Even(m)): @Even(m.wrapping_add(2)),
            Assumed(evidence: @(n == 4)),
        }
        prop Trivial { Intro }",
    );
    let source = sources.get(file);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Prop {
        name,
        parameters,
        variants,
    } = &parsed.program.declarations[0].kind
    else {
        panic!()
    };
    assert_eq!(name.text, "Even");
    assert_eq!(parameters.len(), 1);
    assert_eq!(variants.len(), 3);
    assert_eq!(
        source.slice(variants[0].target.as_ref().unwrap().span),
        Some("Even(0)")
    );
    assert_eq!(variants[1].fields.len(), 2);
    assert_eq!(
        source.slice(variants[1].span),
        Some("Step(m: u8, smaller: @Even(m)): @Even(m.wrapping_add(2))")
    );
    assert!(variants[2].target.is_none());
    let DeclarationKind::Prop {
        parameters,
        variants,
        ..
    } = &parsed.program.declarations[1].kind
    else {
        panic!()
    };
    assert!(parameters.is_empty());
    assert_eq!(variants[0].name.text, "Intro");
}

#[test]
fn match_arms_take_every_pattern_form() {
    let ExprKind::Match { scrutinee, arms } = expression(
        "match value {
            Event::Wrong => 0,
            Event::Code(code) => code,
            And::Intro(_, (left, right)) => left,
            Lock { failures, open: true } => failures,
            (0, false) => { 1 }
            _ => 2
        }",
    )
    .kind
    else {
        panic!()
    };
    assert!(matches!(scrutinee.kind, ExprKind::Name(_)));
    assert_eq!(arms.len(), 6);
    assert!(
        matches!(&arms[0].pattern.kind, PatternKind::Variant { path, arguments: None } if path.prefix.text == "Event" && path.name.text == "Wrong")
    );
    assert!(
        matches!(&arms[1].pattern.kind, PatternKind::Variant { arguments: Some(arguments), .. } if arguments.len() == 1)
    );
    let PatternKind::Variant {
        arguments: Some(arguments),
        ..
    } = &arms[2].pattern.kind
    else {
        panic!()
    };
    assert!(matches!(arguments[0].kind, PatternKind::Wildcard));
    assert!(matches!(arguments[1].kind, PatternKind::Tuple(_)));
    let PatternKind::Struct { name, fields } = &arms[3].pattern.kind else {
        panic!()
    };
    assert_eq!(name.text, "Lock");
    assert!(fields[0].name.is_none());
    assert!(matches!(fields[0].pattern.kind, PatternKind::Name(_)));
    assert_eq!(fields[1].name.as_ref().unwrap().text, "open");
    assert!(matches!(fields[1].pattern.kind, PatternKind::Bool(true)));
    let PatternKind::Tuple(literals) = &arms[4].pattern.kind else {
        panic!()
    };
    assert!(
        matches!(&literals[0].kind, PatternKind::Integer(literal) if literal.value == Natural::zero())
    );
    assert!(matches!(arms[5].pattern.kind, PatternKind::Wildcard));
}

#[test]
fn the_arm_separator_and_implication_share_a_token_without_ambiguity() {
    let ExprKind::Match { arms, .. } =
        expression("match p { Side::Left => prop!(a => b), _ => c }").kind
    else {
        panic!()
    };
    assert_eq!(arms.len(), 2);
    let implication = formula("a => b");
    binary(&implication, BinaryOp::Implies);
    // Outside a formula, an arm's `=>` is the one `=>` there is.
    let parsed = parse_text("fn f(p: Side) -> u8 { match p { Side::Left => a => b, _ => c } }");
    assert_eq!(
        parsed.diagnostics[0].code, "L0119",
        "{:?}",
        parsed.diagnostics
    );
    let parsed = parse_text("fn f(x: u8) -> u8 { match x { 0 -> 1, _ => 2 } }");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("expected `=>`")),
        "{:?}",
        parsed.diagnostics
    );
}

#[test]
fn paths_construct_variants_and_projections_take_names_or_positions() {
    let ExprKind::Call { callee, arguments } = expression("Event::Code(7)").kind else {
        panic!()
    };
    assert!(matches!(&callee.kind, ExprKind::Path(path) if path.name.text == "Code"));
    assert_eq!(arguments.len(), 1);
    assert!(matches!(expression("Event::Wrong").kind, ExprKind::Path(_)));
    let ExprKind::Member { value, name } = expression("pair.0.1.failures").kind else {
        panic!()
    };
    assert_eq!(name.text, "failures");
    let ExprKind::Index { value, index, .. } = value.kind else {
        panic!()
    };
    assert_eq!(index, "1");
    assert!(matches!(&value.kind, ExprKind::Index { index, .. } if index == "0"));
}

#[test]
fn struct_literals_accept_named_and_shorthand_fields() {
    let ExprKind::Struct { name, fields } =
        expression("Lock { failures: n.wrapping_add(1), open, }").kind
    else {
        panic!()
    };
    assert_eq!(name.text, "Lock");
    assert_eq!(fields[0].name.as_ref().unwrap().text, "failures");
    assert!(fields[1].name.is_none());
    assert!(matches!(&fields[1].value.kind, ExprKind::Name(name) if name.text == "open"));
}

#[test]
fn a_name_before_a_header_block_is_not_a_struct_literal() {
    // As in Rust, `if`, `match`, and `for` headers read `name {` as the block.
    let ExprKind::If { condition, .. } = expression("if ready { 1 } else { 2 }").kind else {
        panic!()
    };
    assert!(matches!(condition.kind, ExprKind::Name(_)));
    let ExprKind::Match { scrutinee, .. } = expression("match event { _ => 1 }").kind else {
        panic!()
    };
    assert!(matches!(scrutinee.kind, ExprKind::Name(_)));
    // Delimiters lift the restriction.
    let ExprKind::If { condition, .. } =
        expression("if same(Lock { failures: 0, open }, other) { 1 } else { 2 }").kind
    else {
        panic!()
    };
    assert!(matches!(condition.kind, ExprKind::Call { .. }));
    let ExprKind::Match { scrutinee, .. } = expression("match (Unit {}) { _ => 1 }").kind else {
        panic!()
    };
    assert!(matches!(scrutinee.kind, ExprKind::Group(_)));
    // A proof type is followed by a body in results and loop headers.
    let block = body("fn f(claim: Prop, given: @claim) -> @claim { given }");
    assert!(matches!(block.tail.unwrap().kind, ExprKind::Name(_)));
}

#[test]
fn loops_list_their_state_and_result() {
    let ExprKind::Loop {
        state,
        result,
        body,
    } = expression(
        "loop (i: u8 = 0, bound: @(i <= n) = _) -> (out: u8, @(out == n)) {
            if i == n { break (i, _) } else { continue(i.wrapping_add(1), _) }
        }",
    )
    .kind
    else {
        panic!()
    };
    assert_eq!(state.len(), 2);
    assert_eq!(state[1].name.text, "bound");
    assert!(matches!(state[1].ty.kind, TypeKind::Proof(_)));
    assert!(matches!(state[1].initial.kind, ExprKind::Hole));
    assert!(matches!(result.kind, TypeKind::Tuple(_)));
    let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = body.tail.unwrap().kind
    else {
        panic!()
    };
    assert!(matches!(
        then_branch.tail.unwrap().kind,
        ExprKind::Break(value) if matches!(value.kind, ExprKind::Tuple(_))
    ));
    let ExprKind::Block(else_block) = else_branch.kind else {
        panic!()
    };
    assert!(matches!(
        else_block.tail.unwrap().kind,
        ExprKind::Continue(arguments) if arguments.len() == 2
    ));
    for text in [
        "loop { break 1 }",
        "loop () -> u8 { break }",
        "loop () -> u8 { continue }",
    ] {
        assert!(
            !parse_text(&format!("fn f() -> u8 {{ {text} }}")).is_success(),
            "{text}"
        );
    }
    assert!(matches!(
        expression("loop () -> u8 { continue() }").kind,
        ExprKind::Loop { state, .. } if state.is_empty()
    ));
}

#[test]
fn a_for_header_separates_the_upper_bound_from_the_state_list() {
    let ExprKind::For {
        index,
        lower,
        upper,
        state,
        body,
    } = expression("for i in 0..n (acc: u8 = 0, same: @(acc == i) = _) { continue(acc, same) }")
        .kind
    else {
        panic!()
    };
    assert_eq!(index.text, "i");
    assert!(matches!(&lower.kind, ExprKind::Integer(literal) if literal.value == Natural::zero()));
    assert!(matches!(&upper.kind, ExprKind::Name(name) if name.text == "n"));
    assert_eq!(state.len(), 2);
    assert!(matches!(body.tail.unwrap().kind, ExprKind::Continue(_)));

    // Only the group directly before the body is the state list.
    let ExprKind::For {
        lower,
        upper,
        state,
        ..
    } = expression("for i in start(a)..limit(a, b) () { continue() }").kind
    else {
        panic!()
    };
    assert!(matches!(lower.kind, ExprKind::Call { .. }));
    assert!(matches!(&upper.kind, ExprKind::Call { arguments, .. } if arguments.len() == 2));
    assert!(state.is_empty());

    let parsed = parse_text("fn f(n: u8) -> () { for i in 0..n { continue() } }");
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("lists its state")),
        "{:?}",
        parsed.diagnostics
    );
}

#[test]
fn function_types_record_their_mode_and_parameter_names() {
    let parsed = parse_text(
        "math fn apply(f: math fn(x: u8) -> @(x == x), g: fn(u8, bool) -> u8) -> () { () }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Function { parameters, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    let TypeKind::Function {
        mode,
        parameters: inputs,
        result,
    } = &parameters[0].ty.kind
    else {
        panic!()
    };
    assert_eq!(*mode, FunctionMode::Math);
    assert_eq!(inputs[0].name.as_ref().unwrap().text, "x");
    assert!(matches!(result.kind, TypeKind::Proof(_)));
    assert!(matches!(
        &parameters[1].ty.kind,
        TypeKind::Function { mode: FunctionMode::Runtime, parameters, .. } if parameters.len() == 2
    ));
}

#[test]
fn exists_mirrors_forall() {
    let inner = formula("exists (n: u8, m: u8) { n == m }");
    assert!(matches!(inner.kind, ExprKind::Exists { parameters, .. } if parameters.len() == 2));
    assert!(!parse_text("fn f() -> Prop { prop!(exists () { true }) }").is_success());
}

#[test]
fn let_accepts_constructor_and_struct_patterns() {
    let block = body(
        "fn f(both: @And(p, q), lock: Lock) -> () {
            let And::Intro(left, right) = both;
            let Lock { failures, open: _ } = lock;
            ()
        }",
    );
    let StatementKind::Let { pattern, .. } = &block.statements[0].kind else {
        panic!()
    };
    assert!(matches!(pattern.kind, PatternKind::Variant { .. }));
    let StatementKind::Let { pattern, .. } = &block.statements[1].kind else {
        panic!()
    };
    assert!(matches!(pattern.kind, PatternKind::Struct { .. }));
}

// Locus tokenizes as Rust: keywords, literals, and the tokens Locus does not
// use yet.

fn lex_text(text: &str) -> Lexed {
    let mut sources = SourceMap::default();
    let file = sources.add("test.lc", text);
    lex(sources.get(file))
}

fn kinds(lexed: &Lexed) -> Vec<K> {
    lexed.tokens.iter().map(|token| token.kind).collect()
}

/// The strict keywords of Rust 2024 and then the reserved ones, written out
/// here so that the test does not lean on the lexer's own list.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

#[test]
fn every_rust_keyword_is_reserved_in_every_name_position() {
    assert_eq!(RUST_KEYWORDS.len(), 38 + 14);
    // `true` and `false` are patterns, and `fn` begins a type.
    let positions: &[(&str, &str, &[&str])] = &[
        ("a function", "fn {}() -> u8 { 1 }", &[]),
        ("a parameter", "fn f({}: u8) -> u8 { 1 }", &[]),
        ("a struct", "struct {} { x: u8 }", &[]),
        ("an enum", "enum {} { A }", &[]),
        ("a constant", "const {}: u8 = 1;", &[]),
        ("a field", "struct S { {}: u8 }", &[]),
        ("a variant", "enum E { A, {}(u8) }", &[]),
        ("a proof constructor", "prop P { {}: @(true) }", &[]),
        ("a payload field", "enum E { A({}: u8) }", &[]),
        (
            "a result field",
            "fn f() -> ({}: u8, bool) { (1, true) }",
            &[],
        ),
        ("a type", "fn f(x: {}) -> u8 { 1 }", &["fn"]),
        (
            "a binding",
            "fn f() -> u8 { let {} = 1; 1 }",
            &["true", "false"],
        ),
        (
            "a binding in a tuple",
            "fn f() -> u8 { let (_, {}) = p; 1 }",
            &["true", "false"],
        ),
        (
            "a binding in an arm",
            "fn f() -> u8 { match e { E::A({}) => 1 } }",
            &["true", "false"],
        ),
        (
            "a field pattern",
            "fn f() -> u8 { let S { {}: _ } = s; 1 }",
            &[],
        ),
        ("a field of a literal", "fn f() -> S { S { {}: 1 } }", &[]),
        ("a member", "fn f() -> u8 { s.{} }", &[]),
        ("a variant of a path", "fn f() -> E { E::{} }", &[]),
        (
            "loop state",
            "fn f() -> u8 { loop ({}: u8 = 0) -> u8 { break 1 } }",
            &[],
        ),
        (
            "a loop index",
            "fn f() -> u8 { for {} in 0..1 () { continue() } }",
            &[],
        ),
        (
            "a bound variable",
            "const c: Prop = prop!(forall ({}: u8) { true });",
            &[],
        ),
    ];
    for keyword in RUST_KEYWORDS {
        for (position, template, exempt) in positions {
            if exempt.contains(keyword) {
                continue;
            }
            let text = template.replace("{}", keyword);
            let mut sources = SourceMap::default();
            let file = sources.add("test.lc", text.as_str());
            let source = sources.get(file);
            let parsed = parse(source);
            let first = parsed
                .diagnostics
                .first()
                .unwrap_or_else(|| panic!("`{keyword}` was accepted as {position}: {text}"));
            // The four that stand where names do in Rust are reported as the
            // Rust they are; every other keyword as no name.
            let code = if matches!(*keyword, "self" | "Self" | "crate" | "super") {
                "L0116"
            } else {
                "L0115"
            };
            assert_eq!(first.code, code, "{text}: {}", first.message);
            assert_eq!(source.slice(first.labels[0].span), Some(*keyword), "{text}");
            assert!(
                first.message.contains(&format!("`{keyword}`")),
                "{text}: {}",
                first.message
            );
            // The keyword is reported once, whatever recovery says after it.
            let reports = parsed
                .diagnostics
                .iter()
                .filter(|diagnostic| matches!(diagnostic.code, "L0115" | "L0116"))
                .count();
            assert_eq!(reports, 1, "{text}: {:#?}", parsed.diagnostics);
        }
    }
    let message = &parse_text("fn move(ref: u8) -> u8 { ref }").diagnostics[0].message;
    assert_eq!(
        message,
        "`move` is a Rust keyword and cannot be used as a name"
    );
}

#[test]
fn weak_keywords_and_the_words_of_locus_are_names() {
    let parsed = parse_text(
        "fn union(raw: u8, safe: u8, auto: u8, default: u8) -> u8 { let macro_rules = raw; let math = safe; let prop = auto; macro_rules }
         fn forall(exists: u8, def: u8, prove: u8) -> u8 { let forall = exists; forall }",
    );
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
}

#[test]
fn integer_literals_carry_their_value_and_suffix() {
    use IntegerSuffix as S;
    let mut cases: Vec<(String, Natural, Option<S>)> = Vec::new();
    // The largest value of each fixed-width type, and one past it: the lexer
    // reads the value and leaves the range to the elaborator.
    for (suffix, max) in [
        (S::U8, u128::from(u8::MAX)),
        (S::U16, u128::from(u16::MAX)),
        (S::U32, u128::from(u32::MAX)),
        (S::U64, u128::from(u64::MAX)),
        (S::U128, u128::MAX),
        (S::I8, i8::MAX as u128),
        (S::I16, i16::MAX as u128),
        (S::I32, i32::MAX as u128),
        (S::I64, i64::MAX as u128),
        (S::I128, i128::MAX as u128),
    ] {
        let name = suffix.name();
        let past = Natural::from_u128(max).succ();
        cases.push((
            format!("{max}{name}"),
            Natural::from_u128(max),
            Some(suffix),
        ));
        cases.push((
            format!("{max:#x}_{name}"),
            Natural::from_u128(max),
            Some(suffix),
        ));
        cases.push((format!("{past}{name}"), past.clone(), Some(suffix)));
        cases.push((format!("{past}"), past, None));
    }
    let two_to_128 = Natural::from_u128(u128::MAX).succ();
    for (text, value, suffix) in [
        ("0", Natural::zero(), None),
        ("007", Natural::from(7), None),
        ("255", Natural::from(255), None),
        ("256u8", Natural::from(256), Some(S::U8)),
        ("0xff", Natural::from(255), None),
        ("0xFF_u8", Natural::from(255), Some(S::U8)),
        ("0o377", Natural::from(255), None),
        ("0b1111_1111", Natural::from(255), None),
        ("0b1111_1111i8", Natural::from(255), Some(S::I8)),
        ("1_000", Natural::from(1000), None),
        ("1__0", Natural::from(10), None),
        ("1_", Natural::from(1), None),
        ("0_u8", Natural::zero(), Some(S::U8)),
        ("0x_1_", Natural::from(1), None),
        ("0xdead_beef", Natural::from(0xdead_beef), None),
        ("0x1f32", Natural::from(0x1f32), None),
        ("0xbu8", Natural::from(11), Some(S::U8)),
        ("12usize", Natural::from(12), Some(S::Usize)),
        ("12isize", Natural::from(12), Some(S::Isize)),
        (
            "340282366920938463463374607431768211456u128",
            two_to_128.clone(),
            Some(S::U128),
        ),
        (
            "0x1_0000_0000_0000_0000_0000_0000_0000_0000",
            two_to_128.clone(),
            None,
        ),
        (
            "0o4000000000000000000000000000000000000000000",
            two_to_128.clone(),
            None,
        ),
        (
            "9999999999999999999999999999999999999999",
            "9999999999999999999999999999999999999999".parse().unwrap(),
            None,
        ),
    ] {
        cases.push((text.to_owned(), value, suffix));
    }
    cases.push((format!("0b1{}", "0".repeat(128)), two_to_128, None));

    for (text, value, suffix) in cases {
        let lexed = lex_text(&text);
        assert!(
            lexed.diagnostics.is_empty(),
            "{text}: {:?}",
            lexed.diagnostics
        );
        assert_eq!(kinds(&lexed), [K::Integer, K::Eof], "{text}");
        assert_eq!(
            lexed.literal(lexed.tokens[0]),
            Some(&Literal::Integer(IntegerLiteral { value, suffix })),
            "{text}"
        );
    }
}

#[test]
fn invalid_integer_literals_point_at_what_is_wrong() {
    for (text, offending, message) in [
        ("1abc", "abc", "invalid suffix `abc` for an integer literal"),
        ("0x1G", "G", "invalid suffix `G` for an integer literal"),
        ("1u9", "u9", "invalid suffix `u9` for an integer literal"),
        (
            "1u8u8",
            "u8u8",
            "invalid suffix `u8u8` for an integer literal",
        ),
        ("0X1F", "X1F", "invalid suffix `X1F` for an integer literal"),
        ("1é", "é", "invalid suffix `é` for an integer literal"),
        (
            "1else",
            "else",
            "invalid suffix `else` for an integer literal",
        ),
        ("0b12", "2", "`2` is not a digit of a base 2 literal"),
        ("0o1_8", "8", "`8` is not a digit of a base 8 literal"),
        ("0x", "0x", "no digits after `0x`"),
        ("0b_", "0b_", "no digits after `0b`"),
        ("0ou8", "0ou8", "no digits after `0o`"),
        ("1.0abc", "abc", "invalid suffix `abc` for a float literal"),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let lexed = lex(source);
        assert_eq!(kinds(&lexed), [K::Error, K::Eof], "{text}");
        assert_eq!(lexed.diagnostics.len(), 1, "{text}");
        let diagnostic = &lexed.diagnostics[0];
        assert_eq!(diagnostic.code, "L0003", "{text}");
        assert_eq!(diagnostic.message, message, "{text}");
        assert_eq!(
            source.slice(diagnostic.labels[0].span),
            Some(offending),
            "{text}"
        );
    }
}

#[test]
fn a_dot_after_digits_is_a_float_only_where_rust_reads_one() {
    for (text, expected) in [
        ("pair.0", vec![K::Name, K::Dot, K::Integer]),
        (
            "pair.0.1",
            vec![K::Name, K::Dot, K::Integer, K::Dot, K::Integer],
        ),
        ("0..n", vec![K::Integer, K::DotDot, K::Name]),
        ("0..=9", vec![K::Integer, K::DotDotEqual, K::Integer]),
        (
            "1.max(2)",
            vec![
                K::Integer,
                K::Dot,
                K::Name,
                K::LParen,
                K::Integer,
                K::RParen,
            ],
        ),
        ("0x1.e5", vec![K::Integer, K::Dot, K::Name]),
    ] {
        let lexed = lex_text(text);
        assert!(
            lexed.diagnostics.is_empty(),
            "{text}: {:?}",
            lexed.diagnostics
        );
        let mut found = kinds(&lexed);
        assert_eq!(found.pop(), Some(K::Eof));
        assert_eq!(found, expected, "{text}");
    }
    let nested = expression("pair.0.1");
    let ExprKind::Index { value, index, .. } = &nested.kind else {
        panic!("{nested:?}")
    };
    assert_eq!(index, "1");
    assert!(matches!(&value.kind, ExprKind::Index { index, .. } if index == "0"));
    for text in ["pair.0x1", "pair.0u8", "pair.0_0"] {
        let parsed = parse_text(&format!("fn f() -> u8 {{ {text} }}"));
        assert_eq!(parsed.diagnostics.len(), 1, "{text}");
        assert!(parsed.diagnostics[0].message.contains("tuple position"));
    }
}

#[test]
fn string_literals_decode_rusts_escapes() {
    for (text, value) in [
        (r#""""#, ""),
        (r#""plain text""#, "plain text"),
        (r#""\n\r\t\\\0\'\"""#, "\n\r\t\\\0'\""),
        (r#""\x41\x7f\x00""#, "A\x7f\0"),
        (r#""\u{41}\u{1F4A1}\u{10FFFF}\u{00_41}""#, "A💡\u{10FFFF}A"),
        ("\"two\nlines\"", "two\nlines"),
        ("\"two\r\nlines\"", "two\nlines"),
        ("\"one \\\n      line\"", "one line"),
        ("\"one \\\r\n\t line\"", "one line"),
        (
            "\"// not a comment /* nor this\"",
            "// not a comment /* nor this",
        ),
        ("\"💡 é\"", "💡 é"),
    ] {
        let lexed = lex_text(text);
        assert!(
            lexed.diagnostics.is_empty(),
            "{text}: {:?}",
            lexed.diagnostics
        );
        assert_eq!(kinds(&lexed), [K::String, K::Eof], "{text}");
        assert_eq!(
            lexed.literal(lexed.tokens[0]),
            Some(&Literal::String(value.to_owned())),
            "{text}"
        );
    }
    let parsed = expression(r#""a \"quoted\" word""#);
    assert_eq!(parsed.kind, ExprKind::String("a \"quoted\" word".into()));
}

#[test]
fn string_errors_point_at_the_offending_part() {
    for (text, code, offending, message) in [
        (r#""a\qb""#, "L0007", r"\q", "unknown escape `\\q`"),
        (r#""\x80""#, "L0007", r"\x80", "goes up to `\\x7F`"),
        (r#""\xff""#, "L0007", r"\xff", "goes up to `\\x7F`"),
        (
            r#""\x4""#,
            "L0007",
            r"\x4",
            "exactly two hexadecimal digits",
        ),
        (
            r#""\xZZ""#,
            "L0007",
            r"\x",
            "exactly two hexadecimal digits",
        ),
        (r#""\u41""#, "L0007", r"\u", "with braces"),
        (
            r#""\u{}""#,
            "L0007",
            r"\u{}",
            "at least one hexadecimal digit",
        ),
        (r#""\u{41""#, "L0007", r"\u{41", "closing brace"),
        (r#""\u{1234567}""#, "L0007", r"\u{1234567}", "at most six"),
        (r#""\u{D800}""#, "L0007", r"\u{D800}", "not a surrogate"),
        (
            r#""\u{110000}""#,
            "L0007",
            r"\u{110000}",
            "at most `10FFFF`",
        ),
        (
            r#""never closed"#,
            "L0006",
            "\"",
            "unterminated string literal",
        ),
        (
            "\"ends in a backslash\\",
            "L0006",
            "\"",
            "unterminated string literal",
        ),
        ("'", "L0006", "'", "unterminated character literal"),
        ("'\\n", "L0006", "'", "unterminated character literal"),
        (
            "b\"bytes",
            "L0006",
            "\"",
            "unterminated byte string literal",
        ),
        ("b'x", "L0006", "'", "unterminated byte literal"),
        (
            "r##\"raw\"#",
            "L0006",
            "\"",
            "unterminated raw string literal",
        ),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let lexed = lex(source);
        assert_eq!(kinds(&lexed), [K::Error, K::Eof], "{text}");
        assert_eq!(
            lexed.diagnostics.len(),
            1,
            "{text}: {:?}",
            lexed.diagnostics
        );
        let diagnostic = &lexed.diagnostics[0];
        assert_eq!(diagnostic.code, code, "{text}");
        assert!(
            diagnostic.message.contains(message),
            "{text}: {}",
            diagnostic.message
        );
        assert_eq!(
            source.slice(diagnostic.labels[0].span),
            Some(offending),
            "{text}"
        );
    }
    // Every bad escape of a string is reported, and the string is one token.
    let lexed = lex_text(r#""\q and \w" next"#);
    assert_eq!(lexed.diagnostics.len(), 2);
    assert_eq!(kinds(&lexed), [K::Error, K::Name, K::Eof]);
}

#[test]
fn literal_forms_of_rust_are_lexed_whole_and_reported_as_not_in_locus_yet() {
    for (text, what) in [
        ("'a", "lifetimes and loop labels"),
        ("'static", "lifetimes and loop labels"),
        ("'_", "lifetimes and loop labels"),
        ("'x'", "character literals"),
        ("'1'", "character literals"),
        ("'_'", "character literals"),
        ("' '", "character literals"),
        ("'\"'", "character literals"),
        ("'\\n'", "character literals"),
        ("'\\''", "character literals"),
        ("'\\\\'", "character literals"),
        ("'\\u{1F4A1}'", "character literals"),
        ("'💡'", "character literals"),
        ("'ab'", "character literals"),
        ("b'x'", "byte literals"),
        ("b'\\''", "byte literals"),
        ("b\"bytes \\\" and more\"", "byte strings"),
        ("c\"text\"", "C strings"),
        ("r\"raw \\\"", "raw strings"),
        ("r#\"a \" inside\"#", "raw strings"),
        ("r##\"a \"# inside\"##", "raw strings"),
        ("br\"raw\"", "raw byte strings"),
        ("cr#\"raw\"#", "raw C strings"),
        ("r#type", "raw identifiers"),
        ("1.0", "float literals"),
        ("1.", "float literals"),
        ("1e5", "float literals"),
        ("1E-5", "float literals"),
        ("1_0.0_1e+1_0", "float literals"),
        ("1.5f32", "float literals"),
        ("2f64", "float literals"),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let lexed = lex(source);
        assert_eq!(kinds(&lexed), [K::Error, K::Eof], "{text}");
        assert_eq!(source.slice(lexed.tokens[0].span), Some(text));
        assert_eq!(
            lexed.diagnostics.len(),
            1,
            "{text}: {:?}",
            lexed.diagnostics
        );
        assert_eq!(lexed.diagnostics[0].code, "L0005", "{text}");
        assert_eq!(
            lexed.diagnostics[0].message,
            format!("{what} are not in Locus yet"),
            "{text}"
        );
        // The parser adds nothing to what the lexer said.
        let parsed = parse_text(&format!("fn f() -> u8 {{ {text} }}"));
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:#?}",
            parsed.diagnostics
        );
    }
    // A lifetime and a character literal are told apart as rustc tells them.
    let lexed = lex_text("fn f<'a>(x: &'a u8, y: ('a', 'b'))");
    let messages: Vec<_> = lexed
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.split(" are").next().unwrap())
        .collect();
    assert_eq!(
        messages,
        [
            "lifetimes and loop labels",
            "lifetimes and loop labels",
            "character literals",
            "character literals"
        ]
    );
}

#[test]
fn every_punctuation_token_of_rust_is_a_token() {
    let text = "+ - * / % ^ ! & | && || << >> += -= *= /= %= ^= &= |= <<= >>= = == != > < >= <= @ _ . .. ... ..= , ; : :: -> => <- # $ ? ~ ( ) [ ] { }";
    let lexed = lex_text(text);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    assert_eq!(
        kinds(&lexed),
        [
            K::Plus,
            K::Minus,
            K::Star,
            K::Slash,
            K::Percent,
            K::Caret,
            K::Bang,
            K::And,
            K::Or,
            K::AndAnd,
            K::OrOr,
            K::ShiftLeft,
            K::ShiftRight,
            K::PlusEqual,
            K::MinusEqual,
            K::StarEqual,
            K::SlashEqual,
            K::PercentEqual,
            K::CaretEqual,
            K::AndEqual,
            K::OrEqual,
            K::ShiftLeftEqual,
            K::ShiftRightEqual,
            K::Equal,
            K::EqualEqual,
            K::BangEqual,
            K::Greater,
            K::Less,
            K::GreaterEqual,
            K::LessEqual,
            K::At,
            K::Underscore,
            K::Dot,
            K::DotDot,
            K::DotDotDot,
            K::DotDotEqual,
            K::Comma,
            K::Semicolon,
            K::Colon,
            K::PathSep,
            K::Arrow,
            K::Implies,
            K::LeftArrow,
            K::Hash,
            K::Dollar,
            K::Question,
            K::Tilde,
            K::LParen,
            K::RParen,
            K::LBracket,
            K::RBracket,
            K::LBrace,
            K::RBrace,
            K::Eof
        ]
    );
    // Without spaces the longest spelling wins, as in Rust.
    assert_eq!(
        kinds(&lex_text("a<<=b>>=c<-d..=e...f")),
        [
            K::Name,
            K::ShiftLeftEqual,
            K::Name,
            K::ShiftRightEqual,
            K::Name,
            K::LeftArrow,
            K::Name,
            K::DotDotEqual,
            K::Name,
            K::DotDotDot,
            K::Name,
            K::Eof
        ]
    );
}

#[test]
fn operators_of_rust_are_reported_as_not_in_locus_yet() {
    let mut cases: Vec<(String, &str, String)> = Vec::new();
    for operator in ["+=", "-=", "*=", "/=", "%=", "^=", "&=", "|=", "<<=", ">>="] {
        cases.push((
            format!("x {operator} 1"),
            "L0116",
            format!("compound assignment (`{operator}`) is not in Locus yet"),
        ));
    }
    for (text, code, message) in [
        (
            "x << 1",
            "L0116",
            "the shift operator `<<` is not in Locus yet",
        ),
        (
            "x >> 1",
            "L0116",
            "the shift operator `>>` is not in Locus yet",
        ),
        (
            "x & 1",
            "L0116",
            "references and the `&` operator are not in Locus yet",
        ),
        (
            "&x",
            "L0116",
            "references and the `&` operator are not in Locus yet",
        ),
        (
            "x | 1",
            "L0116",
            "closures, or-patterns, and the `|` operator are not in Locus yet",
        ),
        (
            "|y| y",
            "L0116",
            "closures, or-patterns, and the `|` operator are not in Locus yet",
        ),
        ("x ^ 1", "L0116", "the `^` operator is not in Locus yet"),
        ("f(x)?", "L0116", "the `?` operator is not in Locus yet"),
        ("-x", "L0116", "negation (`-`) is not in Locus yet"),
        (
            "*x",
            "L0116",
            "dereferences and raw pointers (`*`) are not in Locus yet",
        ),
        (
            "$x",
            "L0116",
            "`$` belongs to macros, which are not in Locus yet",
        ),
        (
            "~x",
            "L0116",
            "`~` is a token of Rust with no meaning in Locus",
        ),
        (
            "x <- 1",
            "L0116",
            "`<-` is a token of Rust with no meaning in Locus",
        ),
        ("f(...)", "L0116", "`...` is not in Locus yet"),
        (
            "for i in 0..=9 () { continue() }",
            "L0116",
            "inclusive ranges (`..=`) are not in Locus yet",
        ),
        ("x + 1", "L0112", "`+` is not part of the core language"),
        ("x - 1", "L0112", "`-` is not part of the core language"),
        ("x * 2", "L0112", "`*` is not part of the core language"),
        ("x / 2", "L0112", "`/` is not part of the core language"),
        ("x % 2", "L0112", "`%` is not part of the core language"),
    ] {
        cases.push((text.to_owned(), code, message.to_owned()));
    }
    for (text, code, message) in cases {
        let parsed = parse_text(&format!("fn f(x: u8) -> u8 {{ {text}; x }}"));
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].code, code, "{text}");
        assert_eq!(parsed.diagnostics[0].message, message, "{text}");
    }
    let parsed = parse_text("fn f(x: &u8, y: *mut u8) -> u8 { 1 }");
    assert_eq!(parsed.diagnostics.len(), 1);
    assert!(parsed.diagnostics[0].message.starts_with("references"));
}

#[test]
fn constructs_of_rust_are_reported_as_not_in_locus_yet() {
    for (text, message, declarations) in [
        (
            "impl S { fn get() -> u8 { 1 } }",
            "`impl` blocks and `impl Trait` are not in Locus yet",
            0,
        ),
        (
            "use std::fmt;",
            "`use` declarations are not in Locus yet",
            0,
        ),
        (
            "mod inner { fn f() -> u8 { 1 } }",
            "modules (`mod`) are not in Locus yet",
            0,
        ),
        (
            "trait T { fn f() -> u8; }",
            "traits are not in Locus yet",
            0,
        ),
        (
            "type Byte = u8;",
            "type aliases (`type`) are not in Locus yet",
            0,
        ),
        (
            "static LIMIT: u8 = 3;",
            "`static` items are not in Locus yet",
            0,
        ),
        ("extern crate core;", "`extern` is not in Locus yet", 0),
        (
            "pub fn f() -> u8 { 1 }",
            "visibility (`pub`) is not in Locus yet",
            1,
        ),
        (
            "unsafe fn f() -> u8 { 1 }",
            "`unsafe` is not in Locus yet",
            1,
        ),
        ("async fn f() -> u8 { 1 }", "`async` is not in Locus yet", 1),
        (
            "struct S { pub x: u8 }",
            "visibility (`pub`) is not in Locus yet",
            0,
        ),
        (
            "fn f(mut x: u8) -> u8 { x }",
            "`mut` is not in Locus yet",
            0,
        ),
        (
            "fn f(self) -> u8 { 1 }",
            "`self` and methods are not in Locus yet",
            0,
        ),
        ("fn f(x: Self) -> u8 { 1 }", "`Self` is not in Locus yet", 0),
        (
            "fn f(x: dyn T) -> u8 { 1 }",
            "`dyn` trait objects are not in Locus yet",
            0,
        ),
        (
            "fn f(x: impl T) -> u8 { 1 }",
            "`impl` blocks and `impl Trait` are not in Locus yet",
            0,
        ),
        (
            "fn f<T>(x: T) -> T { x }",
            "generic parameters are not in Locus yet",
            0,
        ),
        (
            "struct S<T> { x: T }",
            "generic parameters are not in Locus yet",
            0,
        ),
        (
            "enum E<T> { A(T) }",
            "generic parameters are not in Locus yet",
            0,
        ),
        (
            "fn f(x: u8) -> u8 { while x < 3 { }; x }",
            "`while` loops are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { return x; }",
            "`return` is not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { unsafe { x } }",
            "`unsafe` is not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { let mut y = x; y }",
            "`mut` is not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { let ref y = x; x }",
            "`ref` bindings are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { x as u8 }",
            "`as` casts are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { move || x; x }",
            "closures (`move`) are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { crate::g(x) }",
            "`crate` paths are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { super::g(x) }",
            "`super` paths are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { async { x }; x }",
            "`async` is not in Locus yet",
            1,
        ),
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].code, "L0116", "{text}");
        assert_eq!(parsed.diagnostics[0].message, message, "{text}");
        assert_eq!(parsed.program.declarations.len(), declarations, "{text}");
    }
    // A keyword Rust reserves without a use is no name, wherever it stands.
    for text in ["fn f() -> u8 { yield }", "abstract fn f() -> u8 { 1 }"] {
        let parsed = parse_text(text);
        assert_eq!(parsed.diagnostics.len(), 1, "{text}");
        assert_eq!(parsed.diagnostics[0].code, "L0115", "{text}");
    }
    // The fix for a keyword used as a name is offered, not applied.
    let parsed = parse_text("fn f(type: u8) -> u8 { 1 }");
    let suggestion = &parsed.diagnostics[0].suggestions[0];
    assert_eq!(suggestion.replacement, "type_");
    assert_eq!(suggestion.applicability, Applicability::MaybeIncorrect);
}

#[test]
fn an_attribute_is_reported_once_and_its_item_is_parsed() {
    for (text, diagnostics, declarations) in [
        ("#[derive(Debug, Clone)] struct S { x: u8 }", 1, 1),
        ("#![allow(unused)] fn f() -> u8 { 1 }", 1, 1),
        ("#[test] #[cfg(any(a, b))] fn f() -> u8 { 1 }", 2, 1),
        ("fn f() -> u8 { 1 } #[trailing]", 1, 1),
        ("fn f() -> u8 { #[inline] let x = 1; x }", 1, 1),
        ("struct S { #[serde(rename = \"y\")] x: u8 }", 1, 0),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let parsed = parse(source);
        assert_eq!(
            parsed.diagnostics.len(),
            diagnostics,
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.program.declarations.len(), declarations, "{text}");
        for diagnostic in &parsed.diagnostics {
            assert_eq!(diagnostic.code, "L0105");
            assert_eq!(diagnostic.message, "attributes are not in Locus yet");
            let covered = source.slice(diagnostic.labels[0].span).unwrap();
            assert!(
                covered.starts_with('#') && covered.ends_with(']'),
                "{covered}"
            );
        }
    }
    let parsed = parse_text("#[never closed fn f() -> u8 { 1 }");
    assert_eq!(parsed.diagnostics[0].code, "L0105");
}

// Built-in forms, and the formulas inside `prop!(...)`, `prove!(...)`, and
// `@(...)`.

#[test]
fn forms_are_a_closed_list_spelled_with_a_bang_and_parentheses() {
    for (text, form, arguments) in [
        ("prop!(n > 0)", Form::Prop, 1),
        ("prove!(out == n.wrapping_add(1))", Form::Prove, 1),
        ("rewrite!(same, small)", Form::Rewrite, 2),
        ("unfold!(nonzero, h)", Form::Unfold, 2),
        ("fold!(nonzero, h)", Form::Fold, 2),
        ("old!(x)", Form::Old, 1),
        ("snapshot!(x)", Form::Snapshot, 1),
        ("recurse!(h, f(n))", Form::Recurse, 2),
        ("assert!(n < 3, \"n is {}\", n)", Form::Assert, 3),
        ("unreachable!()", Form::Unreachable, 0),
        ("todo!(\"later\")", Form::Todo, 1),
        ("panic!(\"no room\")", Form::Panic, 1),
        ("debug_assert!(n < 3)", Form::DebugAssert, 1),
        ("matches!(n, 0)", Form::Matches, 2),
        ("vec!(1, 2, 3)", Form::Vec, 3),
    ] {
        let ExprKind::Form {
            form: parsed,
            arguments: parsed_arguments,
            name_span,
            ..
        } = expression(text).kind
        else {
            panic!("{text}")
        };
        assert_eq!(parsed, form, "{text}");
        assert_eq!(parsed_arguments.len(), arguments, "{text}");
        assert_eq!(name_span.range().len(), form.name().len(), "{text}");
        assert_eq!(Form::from_name(form.name()), Some(form));
    }
    // A form is an expression like any other: a value, an argument, a
    // statement.
    let block = body(
        "fn f(n: u8) -> (out: u8, @(out == n)) {
            prove!(n == n);
            let h = prove!(n == n);
            g(prove!(n == n), prop!(n == n));
            (n, prove!(n == n))
        }",
    );
    assert_eq!(block.statements.len(), 3);
    assert!(matches!(
        &block.statements[0].kind,
        StatementKind::Expression(Expr {
            kind: ExprKind::Form {
                form: Form::Prove,
                ..
            },
            ..
        })
    ));
    // `!=` is one token, and `!` before an operand is negation still.
    let not_equal = expression("a != !b");
    let (_, right) = binary(&not_equal, BinaryOp::NotEqual);
    assert!(matches!(right.kind, ExprKind::Not(_)));
    let negated = formula("!p && !(q)");
    let (left, right) = binary(&negated, BinaryOp::And);
    assert!(matches!(left.kind, ExprKind::Not(_)));
    assert!(matches!(right.kind, ExprKind::Not(_)));
}

#[test]
fn a_form_outside_the_list_or_with_other_delimiters_is_reported() {
    let parsed = parse_text("fn f() -> u8 { foo!(1) }");
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let error = &parsed.diagnostics[0];
    assert_eq!(error.code, "L0118");
    assert_eq!(error.message, "`foo!` is not a form of Locus");
    assert_eq!(
        error.notes,
        [
            "the forms are `prop!`, `prove!`, `rewrite!`, `unfold!`, `fold!`, `old!`, `snapshot!`, `recurse!`, `assert!`, `unreachable!`, `todo!`, `panic!`, `debug_assert!`, `matches!`, and `vec!`; Locus has no user-defined macros"
        ]
    );
    for form in Form::ALL {
        assert!(error.notes[0].contains(&format!("`{}!`", form.name())));
    }
    for text in [
        "vec![1, 2]",
        "prove!{ n == n }",
        "prop![n == n]",
        "todo!{}",
        "println!(\"{}\", n)",
    ] {
        let parsed = parse_text(&format!("fn f(n: u8) -> u8 {{ let x = {text}; n }}"));
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0118")
            .unwrap_or_else(|| panic!("{text}: {:?}", parsed.diagnostics));
        if text.starts_with("println") {
            assert!(
                error.message.contains("not a form"),
                "{text}: {}",
                error.message
            );
        } else {
            assert!(
                error.message.contains("take parentheses"),
                "{text}: {}",
                error.message
            );
        }
        // The rest of the function is still read.
        assert_eq!(parsed.program.declarations.len(), 1, "{text}");
    }
    // `prop!` and `prove!` take one formula, no more and no fewer.
    for text in ["prop!()", "prove!(a, b)", "prop!(,)"] {
        let parsed = parse_text(&format!("fn f() -> u8 {{ {text} }}"));
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0118")
            .unwrap_or_else(|| panic!("{text}: {:?}", parsed.diagnostics));
        assert!(error.message.contains("takes one formula"), "{text}");
    }
}

#[test]
fn quantifiers_and_implication_are_read_only_inside_a_formula() {
    // Inside: the three places a formula is written, nested anywhere in it.
    for text in [
        "prop!(forall (x: u8) { x == x })",
        "prove!(exists (x: u8, y: u8) { x == y => y == x })",
        "prop!(same(forall (x: u8) { true }, p => q) && !(a => b))",
        "prop!(prop!(forall (x: u8) { x <= 255 }))",
        "prop!({ let p = forall (x: u8) { x == x }; p })",
    ] {
        expression(text);
    }
    let expr = formula("forall (n: u8) { n == n => exists (m: u8) { m == n } }");
    let ExprKind::Forall { parameters, body } = expr.kind else {
        panic!()
    };
    assert_eq!(parameters[0].name.text, "n");
    let (_, right) = binary(body.tail.as_ref().unwrap(), BinaryOp::Implies);
    assert!(matches!(right.kind, ExprKind::Exists { .. }));
    let parsed = parse_text("fn f(n: u8) -> @(forall (x: u8) { x <= n => x <= 255 }) { _ }");
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Function { result, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    let TypeKind::Proof(target) = &result.kind else {
        panic!()
    };
    let ExprKind::Group(inner) = &target.kind else {
        panic!("{target:?}")
    };
    assert!(matches!(inner.kind, ExprKind::Forall { .. }));

    // Outside: `forall` and `exists` are names, and `=>` is an error that
    // says where implication is written.
    for text in [
        "fn f(forall: u8, exists: u8) -> u8 { forall.wrapping_add(exists) }",
        "fn forall(n: u8) -> u8 { let exists = n; exists }",
        "fn f() -> u8 { forall(1) }",
        "fn f() -> u8 { let forall = 3; forall }",
        "fn f() -> u8 { exists }",
    ] {
        let parsed = parse_text(text);
        assert!(parsed.is_success(), "{text}: {:?}", parsed.diagnostics);
    }
    let ExprKind::Call { callee, .. } = expression("forall(x)").kind else {
        panic!()
    };
    assert!(matches!(&callee.kind, ExprKind::Name(name) if name.text == "forall"));
    for (text, what) in [
        (
            "fn f(p: Prop, q: Prop) -> Prop { p => q }",
            "`=>` is implication",
        ),
        (
            "fn f() -> u8 { if a => b { 1 } else { 2 } }",
            "`=>` is implication",
        ),
        (
            "fn f() -> u8 { let x = (a => b); 1 }",
            "`=>` is implication",
        ),
        (
            "fn f() -> Prop { forall (x: u8) { x == x } }",
            "`forall (...)` is a quantifier",
        ),
        (
            "fn f() -> Prop { exists (x: u8) { x == x } }",
            "`exists (...)` is a quantifier",
        ),
    ] {
        let parsed = parse_text(text);
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0119")
            .unwrap_or_else(|| panic!("{text}: {:?}", parsed.diagnostics));
        assert!(error.message.contains(what), "{text}: {}", error.message);
        assert!(error.message.contains("only inside a formula"), "{text}");
        assert!(error.notes[0].contains("`prop!(...)`, `prove!(...)`, or `@(...)`"));
    }
}

#[test]
fn the_source_has_no_token_rust_lacks() {
    // Every token kind the lexer produces is a token of Rust. The words of
    // Locus lex as names; `=>` is Rust's fat arrow.
    let lexed = lex_text("math prop def forall exists prove rewrite unfold fold old snapshot");
    assert!(
        lexed.tokens[..lexed.tokens.len() - 1]
            .iter()
            .all(|token| token.kind == K::Name)
    );
    let lexed = lex_text("prop!(a => b) @(c) prove!(d) x![y]");
    assert_eq!(
        kinds(&lexed),
        [
            K::Name,
            K::Bang,
            K::LParen,
            K::Name,
            K::Implies,
            K::Name,
            K::RParen,
            K::At,
            K::LParen,
            K::Name,
            K::RParen,
            K::Name,
            K::Bang,
            K::LParen,
            K::Name,
            K::RParen,
            K::Name,
            K::Bang,
            K::LBracket,
            K::Name,
            K::RBracket,
            K::Eof
        ]
    );
}
