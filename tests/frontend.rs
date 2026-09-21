use locus::ast::{
    BinaryOp, Block, DeclarationKind, Expr, ExprKind, FunctionMode, PatternKind, StatementKind,
    TypeKind,
};
use locus::diagnostic::Applicability;
use locus::lexer::{TokenKind as K, lex};
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
            K::Def,
            K::Name,
            K::Name,
            K::Name,
            K::PathSep,
            K::DotDot,
            K::Match,
            K::In,
            K::Exists,
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
        ("1__2", "L0003"),
        ("12u8", "L0003"),
        ("café", "L0004"),
        ("r#type", "L0005"),
        ("\"a string\"", "L0006"),
        ("💡", "L0001"),
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
    let expr = expression("a.wrapping_add(1) == b && ready => done");
    let (condition, _) = binary(&expr, BinaryOp::Implies);
    let (comparison, _) = binary(condition, BinaryOp::And);
    let (left, _) = binary(comparison, BinaryOp::Equal);
    assert!(matches!(left.kind, ExprKind::Call { .. }));
}

#[test]
fn implication_is_right_associative_and_addition_is_retired() {
    let expr = expression("a => b => c");
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
    let parsed = parse_text("fn f() -> @[true] { @{ reflexivity; } }");
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
        parse_text("fn f(n: u8) -> (out: u8, @[out == n]) { let (value, _) = (n, _); (value, _) }");
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
    assert!(parse_text("fn impossible() -> @[false] { _ }").is_success());
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
            for (open, close) in [
                ("(", ")"),
                ("[", "]"),
                ("{", "}"),
                ("!", ""),
                ("p => ", ""),
                ("f(", ")"),
                ("if c { 1 } else { ", " }"),
                ("match x { _ => ", " }"),
                ("S { x: ", " }"),
                ("for i in 0..n (s: u8 = ", ") { continue(s) }"),
                ("loop () -> u8 { break ", " }"),
            ] {
                let text = format!(
                    "fn f() -> u8 {{ {}1{} }}",
                    open.repeat(512),
                    close.repeat(512)
                );
                assert!(
                    parse_text(&text)
                        .diagnostics
                        .iter()
                        .any(|error| error.code == "L0108"),
                    "{open}"
                );
            }
            let types = format!("fn f() -> {}u8{} {{ 1 }}", "(".repeat(512), ")".repeat(512));
            assert!(
                parse_text(&types)
                    .diagnostics
                    .iter()
                    .any(|error| error.code == "L0108")
            );
            let patterns = format!(
                "fn f() -> u8 {{ let {}x{} = 1; 1 }}",
                "(".repeat(512),
                ")".repeat(512)
            );
            assert!(
                parse_text(&patterns)
                    .diagnostics
                    .iter()
                    .any(|error| error.code == "L0108")
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn malformed_inputs_terminate_and_keep_valid_diagnostic_spans() {
    let alphabet = [
        "fn ", "def ", "const ", "let ", "@", "_", "[", "]", "#", "||", "(", ")", "{", "}", ";",
        "=>", "=", "n", "0", "💡", "é", "\n", "/*", "*/", "math ", "prop ", "struct ", "enum ",
        "match ", "loop ", "for ", "in ", "..", "::", "break ", "continue", ",", ":", ".", "->",
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
        "const reflexive: Prop = [forall (n: u8) { n == n }];
         math fn same(x: u8, y: u8) -> Prop { [x == y] }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Constant { name, ty, value } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert_eq!(name.text, "reflexive");
    assert!(matches!(&ty.kind, TypeKind::Named(name) if name.text == "Prop"));
    assert!(
        matches!(&value.kind, ExprKind::Proposition(inner) if matches!(inner.kind, ExprKind::Forall { .. }))
    );
    let DeclarationKind::Function { result, body, .. } = &parsed.program.declarations[1].kind
    else {
        panic!()
    };
    assert!(matches!(&result.kind, TypeKind::Named(name) if name.text == "Prop"));
    assert!(matches!(
        body.tail.as_ref().unwrap().kind,
        ExprKind::Proposition(_)
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
    let text = "def same(x: u8, y: u8) -> Prop { [x == y] }";
    let parsed = parse_text(text);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(parsed.diagnostics[0].code, "L0113");
    assert_eq!(parsed.program.declarations.len(), 1);
    let fix = &parsed.diagnostics[0].suggestions[0];
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    assert!(parse_text(&fixed).is_success(), "{fixed}");
}

#[test]
fn proof_types_accept_named_inline_and_called_propositions_with_precise_spans() {
    for (target, spelling) in [
        ("claim", "@claim"),
        ("[n == n]", "@ [n == n]"),
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
            "[n == n]" => assert!(matches!(proposition.kind, ExprKind::Proposition(_))),
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

#[test]
fn brackets_always_hold_one_proposition() {
    for text in ["[n > 0]", "[forall (n: u8) { n == n }]", "[[n]]"] {
        assert!(matches!(expression(text).kind, ExprKind::Proposition(_)));
    }
    for text in ["[]", "[n,]", "[n, m]", "[n; 3]"] {
        let parsed = parse_text(&format!("fn f() -> u8 {{ {text} }}"));
        assert!(
            parsed.diagnostics.iter().any(|d| d.code == "L0114"),
            "{text}: {:?}",
            parsed.diagnostics
        );
    }
}

#[test]
fn a_bracket_is_a_proposition_with_or_without_an_annotation() {
    let parsed = parse_text(
        "fn f(n: u8) -> () {
            let claim: Prop = [n > 0];
            let inferred = [n > 0];
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
        assert!(matches!(value.kind, ExprKind::Proposition(_)));
    }
}

#[test]
fn array_types_are_outside_the_core_and_proof_types_are_not() {
    for text in [
        "fn f(xs: [bool; 1]) -> () { () }",
        "fn f(ys: [u8]) -> () { () }",
    ] {
        assert!(
            parse_text(text)
                .diagnostics
                .iter()
                .any(|d| d.code == "L0114")
        );
    }
    assert!(parse_text("fn f(h: @[true]) -> () { () }").is_success());
}

#[test]
fn proposition_operations_have_boolean_style_precedence_and_right_associative_implication() {
    let expr = expression("!p && q || r && s => t => u");
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
        let parsed = parse_text(&format!("fn f(n: u8) -> @[n == n] {{ {text} }}"));
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
        "fn f() -> @[true] { # }",
        "fn f() -> @[true] { #(true) }",
        "fn f() -> @[true] { #{ reflexivity; } }",
    ] {
        let parsed = parse_text(text);
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0111")
            .unwrap();
        assert!(error.message.contains("no longer proof syntax"));
    }
    for text in ["fn f() -> #[true] { _ }", "fn f() -> @[true] { #[true] }"] {
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
    let text = "fn f() -> @[n == n) { _ } const good: Prop = [true];";
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
    let text = "const claim: Prop = [true]\nfn good() -> u8 { 1 }";
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
        "math fn same(x: u8, y: u8) -> Prop { [x == y] }
         math fn keep(p: Prop, h: @p) -> @p { h }
         fn self_equal(n: u8) -> @[same(n, n)] { _ }",
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
         math fn good() -> Prop { [true] }
         const claim: Prop = [true];",
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
            Assumed(evidence: @[n == 4]),
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
    assert!(matches!(&literals[0].kind, PatternKind::Integer(text) if text == "0"));
    assert!(matches!(arms[5].pattern.kind, PatternKind::Wildcard));
}

#[test]
fn the_arm_separator_and_implication_share_a_token_without_ambiguity() {
    let ExprKind::Match { arms, .. } = expression("match p { Side::Left => a => b, _ => c }").kind
    else {
        panic!()
    };
    assert_eq!(arms.len(), 2);
    binary(&arms[0].body, BinaryOp::Implies);
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
        "loop (i: u8 = 0, bound: @[i <= n] = _) -> (out: u8, @[out == n]) {
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
    } = expression("for i in 0..n (acc: u8 = 0, same: @[acc == i] = _) { continue(acc, same) }")
        .kind
    else {
        panic!()
    };
    assert_eq!(index.text, "i");
    assert!(matches!(&lower.kind, ExprKind::Integer(text) if text == "0"));
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
        "math fn apply(f: math fn(x: u8) -> @[x == x], g: fn(u8, bool) -> u8) -> () { () }",
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
    let ExprKind::Proposition(inner) = expression("[exists (n: u8, m: u8) { n == m }]").kind else {
        panic!()
    };
    assert!(matches!(inner.kind, ExprKind::Exists { parameters, .. } if parameters.len() == 2));
    assert!(!parse_text("fn f() -> Prop { [exists () { true }] }").is_success());
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
