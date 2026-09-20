use locus::ast::{
    BinaryOp, DeclarationKind, Expr, ExprKind, FunctionMode, PatternKind, ProofRequest,
    StatementKind, TypeKind,
};
use locus::diagnostic::Applicability;
use locus::lexer::{TokenKind as K, lex};
use locus::parser::{Parsed, parse};
use locus::source::{FileId, SourceMap, Span};

fn parse_text(text: &str) -> Parsed {
    let mut sources = SourceMap::default();
    let file = sources.add("test.loc", text);
    parse(sources.get(file))
}

fn expression(text: &str) -> Expr {
    let parsed = parse_text(&format!("fn example() -> Nat {{ {text} }}"));
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
    let file = sources.add("unicode.loc", "aé\r\nb\n");
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
    let text = "fn fn_name forall_ _ 1_000 9999999999999999999999999999999999999999 == => -> != <= >= && || @ # const def Prop prop";
    let mut sources = SourceMap::default();
    let file = sources.add("test.loc", text);
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
    let file = sources.add("test.loc", "/* outer /* inner */ end */ fn // tail\n x");
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
        let file = sources.add("test.loc", text);
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
        include_str!("../examples/increment.loc"),
        include_str!("../examples/preserve.loc"),
        include_str!("../examples/proofs.loc"),
        include_str!("../examples/propositions.loc"),
        include_str!("../examples/machine.loc"),
    ] {
        let parsed = parse_text(example);
        assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    }
}

#[test]
fn addition_equality_conjunction_and_implication_have_distinct_precedence() {
    let expr = expression("a + 1 == b + 1 && ready => done");
    let (condition, _) = binary(&expr, BinaryOp::Implies);
    let (comparison, _) = binary(condition, BinaryOp::And);
    let (left, right) = binary(comparison, BinaryOp::Equal);
    binary(left, BinaryOp::Add);
    binary(right, BinaryOp::Add);
}

#[test]
fn addition_is_left_associative_and_implication_is_right_associative() {
    let expr = expression("a + b + c");
    let (left, _) = binary(&expr, BinaryOp::Add);
    binary(left, BinaryOp::Add);
    let expr = expression("a => b => c");
    let (_, right) = binary(&expr, BinaryOp::Implies);
    binary(right, BinaryOp::Implies);
}

#[test]
fn comparisons_reject_chaining_but_allow_explicit_grouping() {
    for text in ["a == b == c", "a < b <= c", "a != b > c"] {
        let parsed = parse_text(&format!("fn f() -> Bool {{ {text} }}"));
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
fn proof_forms_have_distinct_ast_nodes() {
    assert!(matches!(
        expression("_").kind,
        ExprKind::Proof(ProofRequest::Inferred)
    ));
    let ExprKind::Proof(ProofRequest::Block { commands }) =
        expression("@ { rewrite equal; reflexivity; }").kind
    else {
        panic!()
    };
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].name.text, "rewrite");
    assert_eq!(commands[0].arguments.len(), 1);
    assert!(commands[1].arguments.is_empty());
}

#[test]
fn dependent_results_and_destructuring_keep_their_binders() {
    let parsed = parse_text(
        "fn f(n: Nat) -> (out: Nat, @[out == n]) { let (value, _) = (n, _); (value, _) }",
    );
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
    let text = "fn f() -> Nat { let x = 1\n let y = 2; y }";
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
    let text = "fn f() -> (out: Nat) { (1,) }";
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
    let parsed = parse_text(
        "fn broken( {} fn good() -> Nat { 1 } fn bad() -> Nat { let x = ; let y = ; 2 }",
    );
    assert!(!parsed.is_success());
    assert_eq!(parsed.program.declarations.len(), 2, "{:?}", parsed);
    assert_eq!(parsed.diagnostics.len(), 3, "{:?}", parsed.diagnostics);
}

#[test]
fn unmatched_delimiter_points_back_to_its_opening() {
    let parsed = parse_text("fn f() -> Nat { (1");
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
        let parsed = parse_text(&format!("{prefix} fn f() -> Nat {{ 1 }}"));
        assert_eq!(parsed.diagnostics[0].code, "L0105");
        assert_eq!(parsed.program.declarations.len(), 1);
    }
}

#[test]
fn if_requires_else_and_else_if_is_supported() {
    let parsed = parse_text("fn f() -> Nat { if true { 1 } }");
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
    let file = sources.add("example.loc", "fn f() -> Nat {\n    let n = 1\n    n\n}\n");
    let parsed = parse(sources.get(file));
    let rendered = parsed.diagnostics[0].render(&sources, false);
    assert!(rendered.contains("error[L0102]"), "{rendered}");
    assert!(rendered.contains("example.loc:2:"), "{rendered}");
    assert!(rendered.contains("let n = 1"), "{rendered}");
    assert!(rendered.contains("insert `;` here"), "{rendered}");
    assert!(!rendered.contains('\u{1b}'));
}

#[test]
fn deeply_nested_input_reports_a_limit_instead_of_overflowing_the_stack() {
    let text = format!(
        "fn f() -> Nat {{ {}1{} }}",
        "(".repeat(512),
        ")".repeat(512)
    );
    assert!(
        parse_text(&text)
            .diagnostics
            .iter()
            .any(|error| error.code == "L0108")
    );
    let text = format!("fn f() -> Nat {{ {}1 }}", "1 + ".repeat(512));
    assert!(
        parse_text(&text)
            .diagnostics
            .iter()
            .any(|error| error.code == "L0108")
    );
}

#[test]
fn malformed_inputs_terminate_and_keep_valid_diagnostic_spans() {
    let alphabet = [
        "fn ", "def ", "const ", "let ", "@", "_", "[", "]", "#", "||", "(", ")", "{", "}", ";",
        "=>", "=", "n", "0", "💡", "é", "\n", "/*", "*/",
    ];
    let mut seed = 17u64;
    for length in 0..256 {
        let mut text = String::new();
        for _ in 0..length % 64 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            text.push_str(alphabet[(seed >> 32) as usize % alphabet.len()]);
        }
        let mut sources = SourceMap::default();
        let file = sources.add("fuzz.loc", &text);
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
fn constants_and_logical_definitions_replace_prop_declarations() {
    let parsed = parse_text(
        "const reflexive: Prop = [forall (n: Nat) { n == n }];
         def same(x: Nat, y: Nat) -> Prop { [x == y] }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Constant { name, ty, value } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert_eq!(name.text, "reflexive");
    assert!(matches!(&ty.kind, TypeKind::Named(name) if name.text == "Prop"));
    assert!(
        matches!(&value.kind, ExprKind::Bracket(inner) if matches!(inner.kind, ExprKind::Forall { .. }))
    );
    let DeclarationKind::Function { result, body, .. } = &parsed.program.declarations[1].kind
    else {
        panic!()
    };
    assert!(matches!(&result.kind, TypeKind::Named(name) if name.text == "Prop"));
    assert!(matches!(
        body.tail.as_ref().unwrap().kind,
        ExprKind::Bracket(_)
    ));
    assert!(!parse_text("prop Same(x: Nat) = x == x;").is_success());
    // `prop` is no longer a declaration keyword.
    assert!(parse_text("def prop() -> Prop { [true] }").is_success());
}

#[test]
fn proof_types_accept_named_inline_and_called_propositions_with_precise_spans() {
    for (target, spelling) in [
        ("claim", "@claim"),
        ("[n == n]", "@ [n == n]"),
        ("same(n, n)", "@same(n, n)"),
    ] {
        let mut sources = SourceMap::default();
        let text = format!("fn f(n: Nat) -> {spelling} {{ _ }}");
        let file = sources.add("proof-type.loc", text);
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
            "[n == n]" => assert!(matches!(proposition.kind, ExprKind::Bracket(_))),
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
    assert!(matches!(
        values[1].kind,
        ExprKind::Proof(ProofRequest::Inferred)
    ));
}

#[test]
fn bracket_syntax_preserves_contextual_literals_and_explicit_arrays() {
    for text in ["[n > 0]", "[forall (n: Nat) { n == n }]", "[[n]]"] {
        assert!(matches!(expression(text).kind, ExprKind::Bracket(_)));
    }
    for (text, count) in [("[]", 0), ("[n,]", 1), ("[n, m]", 2), ("[n, m,]", 2)] {
        assert!(
            matches!(expression(text).kind, ExprKind::Array(elements) if elements.len() == count)
        );
    }
    let ExprKind::RepeatArray { value, count } = expression("[n; 3]").kind else {
        panic!()
    };
    assert!(matches!(value.kind, ExprKind::Name(_)));
    assert!(matches!(&count.kind, ExprKind::Integer(n) if n == "3"));
}

#[test]
fn bracket_interpretation_is_deferred_even_when_annotation_is_present() {
    let parsed = parse_text(
        "fn f(n: Nat) -> () {
            let claim: Prop = [n > 0];
            let flags: [Bool; 1] = [n > 0];
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
        assert!(matches!(value.kind, ExprKind::Bracket(_)));
    }
}

#[test]
fn array_and_slice_type_syntax_remain_distinct_from_proof_types() {
    let parsed = parse_text("fn f(xs: [Bool; 1], ys: [Nat], h: @[true]) -> () { () }");
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Function { parameters, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert!(matches!(parameters[0].ty.kind, TypeKind::Array { .. }));
    assert!(matches!(parameters[1].ty.kind, TypeKind::Slice(_)));
    assert!(matches!(parameters[2].ty.kind, TypeKind::Proof(_)));
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
        let parsed = parse_text(&format!("fn f(n: Nat) -> @[n == n] {{ {text} }}"));
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
        assert!(!parse_text(&format!("fn f() -> Nat {{ {text} }}")).is_success());
    }
}

#[test]
fn missing_constant_semicolon_fix_and_recovery_work() {
    let text = "const claim: Prop = [true]\nfn good() -> Nat { 1 }";
    let parsed = parse_text(text);
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    assert_eq!(parsed.program.declarations.len(), 1);
    let fix = &parsed.diagnostics[0].suggestions[0];
    let mut fixed = text.to_owned();
    fixed.replace_range(fix.span.range(), &fix.replacement);
    assert!(parse_text(&fixed).is_success());
}

#[test]
fn proof_block_missing_brace_does_not_consume_the_next_function() {
    let parsed = parse_text("fn broken() -> @[true] { @{ exact\nfn good() -> Nat { 1 }");
    assert!(!parsed.is_success());
    assert!(
        parsed.program.declarations.iter().any(
            |d| matches!(&d.kind, DeclarationKind::Function { name, .. } if name.text == "good")
        )
    );
}

#[test]
fn syntax_parser_does_not_pretend_to_enforce_prop_or_hole_types() {
    // These will be semantic errors once typing exists. A syntax-only parse must
    // not be presented as checking the Bool/Prop boundary or a proof obligation.
    for text in [
        "fn f(n: Nat) -> Prop { n > 0 }",
        "fn f() -> Nat { _ }",
        "fn f(n: Nat) -> () { let claim: Prop = n > 0; () }",
        "fn f(n: Nat) -> @(n > 0) { _ }",
    ] {
        assert!(parse_text(text).is_success(), "{text}");
    }
}

#[test]
fn executable_and_logical_declarations_preserve_their_modes() {
    let parsed = parse_text(
        "def same(x: Nat, y: Nat) -> Prop { [x == y] }
         def keep(p: Prop, h: @p) -> @p { h }
         fn self_equal(n: Nat) -> @[same(n, n)] { _ }",
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
            FunctionMode::Logical,
            FunctionMode::Logical,
            FunctionMode::Runtime
        ]
    );
}

#[test]
fn recovery_keeps_logical_definitions_after_a_broken_function() {
    let parsed = parse_text(
        "fn broken() -> Nat { 1
         def good() -> Prop { [true] }
         const claim: Prop = [true];",
    );
    assert!(!parsed.is_success());
    assert_eq!(parsed.program.declarations.len(), 2, "{parsed:?}");
    assert!(matches!(
        parsed.program.declarations[0].kind,
        DeclarationKind::Function {
            mode: FunctionMode::Logical,
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
    let parsed = parse_text(include_str!("../examples/proofs.loc"));
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
    let (input, one) = binary(&arguments[1], BinaryOp::Add);
    assert!(matches!(&input.kind, ExprKind::Name(name) if name.text == "n"));
    assert!(matches!(&one.kind, ExprKind::Integer(value) if value == "1"));
}
