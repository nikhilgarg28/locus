use locus::ast::{
    BinaryOp, Block, DeclarationKind, Expr, ExprKind, Form, IntegerLiteral, IntegerSuffix,
    PatternKind, RangeKind, StatementKind, Type, TypeKind,
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
fn implication_is_right_associative_and_arithmetic_parses() {
    let expr = formula("a => b => c");
    let (_, right) = binary(&expr, BinaryOp::Implies);
    binary(right, BinaryOp::Implies);
    // Arithmetic is the parser's since S3; the elaborator says what it means.
    let sum = expression("a + 1");
    let (left, right) = binary(&sum, BinaryOp::Add);
    assert!(matches!(left.kind, ExprKind::Name(_)));
    assert!(matches!(right.kind, ExprKind::Integer(_)));
}

/// Every expression in the table below, parsed and then printed with the
/// parentheses that make its grouping explicit. Source parentheses are not
/// printed, so `(a + b) * c` and `a + b * c` are told apart by their
/// grouping alone.
fn grouped(expr: &Expr) -> String {
    let list = |items: &[Expr]| items.iter().map(grouped).collect::<Vec<_>>().join(", ");
    match &expr.kind {
        ExprKind::Name(name) => name.text.clone(),
        ExprKind::Path(path) => path.text(),
        ExprKind::Integer(literal) => match literal.suffix {
            Some(suffix) => format!("{}{}", literal.value, suffix.name()),
            None => literal.value.to_string(),
        },
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Unit => "()".into(),
        ExprKind::Group(inner) => grouped(inner),
        ExprKind::Tuple(items) => format!("({},)", list(items)),
        ExprKind::Not(inner) => format!("(!{})", grouped(inner)),
        ExprKind::Unary { operator, expr, .. } => {
            format!("({}{})", operator.spelling(), grouped(expr))
        }
        ExprKind::Binary {
            operator,
            left,
            right,
            ..
        } => format!(
            "({} {} {})",
            grouped(left),
            operator.spelling(),
            grouped(right)
        ),
        ExprKind::Cast { expr, ty, .. } => format!("({} as {})", grouped(expr), grouped_ty(ty)),
        ExprKind::Call { callee, arguments } => {
            format!("{}({})", grouped(callee), list(arguments))
        }
        ExprKind::Member { value, name } => format!("{}.{}", grouped(value), name.text),
        ExprKind::Index { value, index, .. } => format!("{}.{index}", grouped(value)),
        ExprKind::Ref { mutable, expr } => {
            format!("(&{}{})", if *mutable { "mut " } else { "" }, grouped(expr))
        }
        ExprKind::Range { kind, lower, upper } => {
            format!("({}{}{})", grouped(lower), kind.spelling(), grouped(upper))
        }
        ExprKind::Return(None) => "return".into(),
        ExprKind::Return(Some(value)) => format!("(return {})", grouped(value)),
        ExprKind::Break(None) => "break".into(),
        ExprKind::Break(Some(value)) => format!("(break {})", grouped(value)),
        ExprKind::Continue(None) => "continue".into(),
        ExprKind::Continue(Some(next)) => format!("continue({})", list(next)),
        // The forms that end in a block, by name: their insides are
        // rendered by `rendered_statements`.
        ExprKind::Block(_) => "<block>".into(),
        ExprKind::If { .. } => "<if>".into(),
        ExprKind::Match { .. } => "<match>".into(),
        ExprKind::Loop { result: None, .. } => "<loop>".into(),
        ExprKind::Loop { .. } => "<loop (state)>".into(),
        ExprKind::While { pattern: None, .. } => "<while>".into(),
        ExprKind::While { .. } => "<while let>".into(),
        ExprKind::For { .. } => "<for>".into(),
        other => panic!("the table has no expression like {other:?}"),
    }
}

fn grouped_ty(ty: &Type) -> String {
    match &ty.kind {
        TypeKind::Named(name) => name.text.clone(),
        TypeKind::Path { path, arguments } if arguments.is_empty() => path.text(),
        TypeKind::Path { path, arguments } => format!(
            "{}<{}>",
            path.text(),
            arguments
                .iter()
                .map(grouped_ty)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TypeKind::Proof(claim) => format!("@({})", grouped(claim)),
        TypeKind::Ref { mutable, inner } => {
            format!(
                "&{}{}",
                if *mutable { "mut " } else { "" },
                grouped_ty(inner)
            )
        }
        TypeKind::Never => "!".into(),
        TypeKind::Unit => "()".into(),
        TypeKind::Group(inner) => format!("({})", grouped_ty(inner)),
        TypeKind::Tuple(fields) => format!(
            "({})",
            fields
                .iter()
                .map(|field| match &field.name {
                    Some(name) => format!("{}: {}", name.text, grouped_ty(&field.ty)),
                    None => grouped_ty(&field.ty),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => panic!("the table has no type like {other:?}"),
    }
}

/// A compact, one-line rendering of an item: what the parser kept of its
/// doc comments, attributes, visibility, and shape, without spans. The
/// snapshot tests of the item forms compare against this.
fn rendered_item(declaration: &locus::ast::Declaration) -> String {
    use locus::ast::{AttributeKind, VariantShape, VisibilityScope};
    let mut out = String::new();
    for doc in &declaration.doc {
        out.push_str(&format!("doc({:?}) ", doc.text));
    }
    for attribute in &declaration.attributes {
        out.push_str(&match &attribute.kind {
            AttributeKind::Terminates { decreases: None } => "#[terminates] ".to_string(),
            AttributeKind::Terminates {
                decreases: Some(measure),
            } => format!("#[terminates(decreases = {})] ", grouped(measure)),
            AttributeKind::Derive(traits) => format!(
                "#[derive({})] ",
                traits
                    .iter()
                    .map(locus::ast::Path::text)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => format!("#[{}] ", other.name()),
        });
    }
    let visibility = |visibility: &Option<locus::ast::Visibility>| match visibility {
        None => String::new(),
        Some(visibility) => match &visibility.scope {
            VisibilityScope::Public => "pub ".into(),
            VisibilityScope::Crate => "pub(crate) ".into(),
            VisibilityScope::Super => "pub(super) ".into(),
            VisibilityScope::SelfModule => "pub(self) ".into(),
            VisibilityScope::In(path) => format!("pub(in {}) ", path.text()),
        },
    };
    out.push_str(&visibility(&declaration.visibility));
    let fields = |fields: &[locus::ast::TypeField]| {
        fields
            .iter()
            .map(|field| match &field.name {
                Some(name) => format!("{}: {}", name.text, grouped_ty(&field.ty)),
                None => grouped_ty(&field.ty),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    match &declaration.kind {
        DeclarationKind::Function {
            name,
            self_param,
            parameters,
            result,
            body,
        } => {
            let mut params: Vec<String> = self_param
                .iter()
                .map(|param| param.kind.spelling().to_string())
                .collect();
            params.extend(parameters.iter().map(|param| {
                format!(
                    "{}{}: {}",
                    if param.mutable { "mut " } else { "" },
                    param.name.text,
                    grouped_ty(&param.ty)
                )
            }));
            out.push_str(&format!(
                "fn {}({}) -> {} {{ {} statement(s) }}",
                name.text,
                params.join(", "),
                grouped_ty(result),
                body.statements.len() + usize::from(body.tail.is_some()),
            ));
        }
        DeclarationKind::Struct { name, fields } => {
            let fields: Vec<String> = fields
                .iter()
                .map(|field| {
                    format!(
                        "{}{}{}: {}",
                        field
                            .doc
                            .iter()
                            .map(|doc| format!("doc({:?}) ", doc.text))
                            .collect::<String>(),
                        visibility(&field.visibility),
                        field.name.text,
                        grouped_ty(&field.ty)
                    )
                })
                .collect();
            out.push_str(&format!("struct {} {{ {} }}", name.text, fields.join(", ")));
        }
        DeclarationKind::Enum { name, variants } => {
            let variants: Vec<String> = variants
                .iter()
                .map(|variant| {
                    let doc: String = variant
                        .doc
                        .iter()
                        .map(|doc| format!("doc({:?}) ", doc.text))
                        .collect();
                    match variant.shape {
                        VariantShape::Unit => format!("{doc}{}", variant.name.text),
                        VariantShape::Tuple => {
                            format!("{doc}{}({})", variant.name.text, fields(&variant.fields))
                        }
                        VariantShape::Struct => {
                            format!(
                                "{doc}{} {{ {} }}",
                                variant.name.text,
                                fields(&variant.fields)
                            )
                        }
                    }
                })
                .collect();
            out.push_str(&format!("enum {} {{ {} }}", name.text, variants.join(", ")));
        }
        DeclarationKind::Prop {
            name,
            parameters,
            variants,
        } => {
            let parameters: Vec<String> = parameters
                .iter()
                .map(|param| format!("{}: {}", param.name.text, grouped_ty(&param.ty)))
                .collect();
            let variants: Vec<String> = variants
                .iter()
                .map(|variant| {
                    let mut text: String = variant
                        .doc
                        .iter()
                        .map(|doc| format!("doc({:?}) ", doc.text))
                        .collect();
                    text.push_str(&variant.name.text);
                    if !variant.fields.is_empty() {
                        text.push_str(&format!("({})", fields(&variant.fields)));
                    }
                    if let Some(target) = &variant.target {
                        text.push_str(&format!(": @{}", grouped(target)));
                    }
                    text
                })
                .collect();
            out.push_str(&format!(
                "prop {}({}) {{ {} }}",
                name.text,
                parameters.join(", "),
                variants.join(", ")
            ));
        }
        DeclarationKind::Constant { name, ty, value } => {
            out.push_str(&format!(
                "const {}: {} = {};",
                name.text,
                grouped_ty(ty),
                grouped(value)
            ));
        }
        DeclarationKind::Impl { target, methods } => {
            let methods: Vec<String> = methods.iter().map(rendered_item).collect();
            out.push_str(&format!(
                "impl {} {{ {} }}",
                target.text(),
                methods.join(" ")
            ));
        }
    }
    out
}

/// The item forms, each parsed from a small source and rendered.
#[test]
fn every_item_form_renders_from_its_syntax_tree() {
    for (source, expected) in [
        (
            "fn f(n: u8) -> u8 { n }",
            "fn f(n: u8) -> u8 { 1 statement(s) }",
        ),
        (
            "#[terminates] #[no_panic] #[no_io] pub fn f(n: u8) -> Prop { prop!(n <= 3) }",
            "#[terminates] #[no_panic] #[no_io] pub fn f(n: u8) -> Prop { 1 statement(s) }",
        ),
        (
            "/// doc\n#[terminates] #[no_panic] #[no_alloc] #[no_io]\npub(crate) fn f(n: u8) -> (out: u8, @(out == n)) { let x = n; (x, _) }",
            "doc(\" doc\") #[terminates] #[no_panic] #[no_alloc] #[no_io] pub(crate) fn f(n: u8) -> (out: u8, @((out == n))) { 2 statement(s) }",
        ),
        (
            "#[terminates(decreases = n - 1)] pub(super) fn f(n: u8) -> u8 { n }",
            "#[terminates(decreases = (n - 1))] pub(super) fn f(n: u8) -> u8 { 1 statement(s) }",
        ),
        (
            "pub(in crate::verified) fn f() -> u8 { 1 }",
            "pub(in crate::verified) fn f() -> u8 { 1 statement(s) }",
        ),
        (
            "pub(self) fn f() -> u8 { 1 }",
            "pub(self) fn f() -> u8 { 1 statement(s) }",
        ),
        (
            "#[derive(Clone, Copy)]\npub struct Lock { /// count\n pub failures: u8, pub(crate) open: bool, secret: u8 }",
            "#[derive(Clone, Copy)] pub struct Lock { doc(\" count\") pub failures: u8, pub(crate) open: bool, secret: u8 }",
        ),
        (
            "struct Percent { value: u32, in_range: @(value <= 100) }",
            "struct Percent { value: u32, in_range: @((value <= 100)) }",
        ),
        (
            "pub enum Shape { Point, /// a pair\n Pair(u8, bool), Named(x: u8, y: u8), Box { width: u8, height: u8 }, }",
            "pub enum Shape { Point, doc(\" a pair\") Pair(u8, bool), Named(x: u8, y: u8), Box { width: u8, height: u8 } }",
        ),
        (
            "prop Within(n: u8) { /// small\n Small: @Within(0), Next(m: u8, @Within(m)) }",
            "prop Within(n: u8) { doc(\" small\") Small: @Within(0), Next(m: u8, @(Within(m))) }",
        ),
        (
            "/// three\n#[no_panic] pub const LIMIT: u8 = 3;",
            "doc(\" three\") #[no_panic] pub const LIMIT: u8 = 3;",
        ),
        (
            "impl Percent {\n    /// make one\n    #[terminates] pub fn new(value: u32) -> Self { Self { value } }\n    fn get(&self) -> u32 { self.value }\n    fn take(self) -> u32 { self.value }\n    fn bump(&mut self, by: u32) -> () { () }\n    pub(crate) fn drain(mut self) -> u32 { self.value }\n}",
            "impl Percent { doc(\" make one\") #[terminates] pub fn new(value: u32) -> Self { 1 statement(s) } fn get(&self) -> u32 { 1 statement(s) } fn take(self) -> u32 { 1 statement(s) } fn bump(&mut self, by: u32) -> () { 1 statement(s) } pub(crate) fn drain(mut self) -> u32 { 1 statement(s) } }",
        ),
        ("impl outer::Percent { }", "impl outer::Percent {  }"),
        (
            "fn f(x: Option<Percent>, y: Ghost<Option<u8>>, z: a::b::C<u8, D>) -> Vec<u8> { x }",
            "fn f(x: Option<Percent>, y: Ghost<Option<u8>>, z: a::b::C<u8, D>) -> Vec<u8> { 1 statement(s) }",
        ),
        (
            "fn f(x: crate::a::B, y: super::C, z: self::D) -> u8 { 1 }",
            "fn f(x: crate::a::B, y: super::C, z: self::D) -> u8 { 1 statement(s) }",
        ),
    ] {
        let parsed = parse_text(source);
        assert!(parsed.is_success(), "{source}: {:#?}", parsed.diagnostics);
        assert_eq!(parsed.program.declarations.len(), 1, "{source}");
        assert_eq!(
            rendered_item(&parsed.program.declarations[0]),
            expected,
            "{source}"
        );
    }
}

/// `impl` blocks: what they hold, `Self` and `self` inside them, and what
/// they reject.
#[test]
fn impl_blocks_hold_methods_and_associated_functions() {
    let parsed = parse_text(
        "impl S { fn f(&self) -> u8 { self.x } fn g() -> Self { Self { x: 1 } } fn h(n: u8) -> u8 { Self::f(n) } }",
    );
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let DeclarationKind::Impl { methods, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert_eq!(methods.len(), 3);
    for (text, message) in [
        (
            "impl S { struct T { x: u8 } }",
            "an `impl` block holds functions: `fn`",
        ),
        (
            "impl S { const N: u8 = 1; }",
            "an `impl` block holds functions: `fn`",
        ),
        (
            "impl S { fn f(n: u8, self) -> u8 { 1 } }",
            "a `self` parameter comes first",
        ),
        (
            "impl S { fn f() -> u8 { self.x } }",
            "`self` is the receiver of a method, and this function has no `self` parameter",
        ),
        (
            "fn f() -> u8 { self.x }",
            "`self` is the receiver of a method, and this function has no `self` parameter",
        ),
        (
            "fn f(&self) -> u8 { 1 }",
            "a `self` parameter belongs to a method in an `impl` block",
        ),
        (
            "fn f() -> Self { 1 }",
            "`Self` is the type of an `impl` block, and this is outside one",
        ),
        (
            "fn f() -> u8 { Self::g() }",
            "`Self` is the type of an `impl` block, and this is outside one",
        ),
        ("impl { }", "expected the type an `impl` block is for"),
    ] {
        let parsed = parse_text(text);
        let first = parsed
            .diagnostics
            .first()
            .unwrap_or_else(|| panic!("{text}"));
        assert_eq!(first.code, "L0100", "{text}");
        assert_eq!(first.message, message, "{text}");
    }
    // A method that fails is recovered on its own, and the block closes.
    let parsed =
        parse_text("impl S { fn f(n: ) -> u8 { 1 } fn g() -> u8 { 2 } } fn h() -> u8 { 3 }");
    assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.program.declarations.len(), 2);
    let DeclarationKind::Impl { methods, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert_eq!(methods.len(), 1);
    // `Self` is a type only inside the block; method calls parse as before.
    let parsed =
        parse_text("impl S { fn f(&self) -> u8 { self.g(1).h() } } fn k(s: S) -> u8 { s.f() }");
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
}

/// Variants with named fields in declarations, patterns, and expressions;
/// `..` in struct patterns.
#[test]
fn named_field_variants_parse_in_every_position() {
    let parsed = parse_text(
        "enum E { V { a: u8, b: bool }, W }
         fn f(e: E) -> u8 {
             let E::V { a, b: _ } = e;
             let E::V { a: x, .. } = e;
             let S { x, .. } = s;
             match e { E::V { a, b } => a, E::V { .. } => 0, E::W => 1 }
         }
         fn g() -> E { E::V { a: 1, b: true } }
         fn h() -> E { if e == E::W { E::V { a, b } } else { E::W } }",
    );
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let DeclarationKind::Function { body, .. } = &parsed.program.declarations[1].kind else {
        panic!()
    };
    let StatementKind::Let { pattern, .. } = &body.statements[1].kind else {
        panic!()
    };
    let PatternKind::Struct { path, fields, rest } = &pattern.kind else {
        panic!("{pattern:?}")
    };
    assert_eq!(path.text(), "E::V");
    assert_eq!(fields.len(), 1);
    assert!(rest.is_some());
    let expr = expression("E::V { a: 1, b: true }");
    let ExprKind::Struct { path, fields } = &expr.kind else {
        panic!("{expr:?}")
    };
    assert_eq!(path.text(), "E::V");
    assert_eq!(fields.len(), 2);
    // `..` comes last, and a variant's braces hold `name: Type` pairs.
    let parsed = parse_text("fn f() -> u8 { let S { .., x } = s; 1 }");
    assert_eq!(parsed.diagnostics[0].code, "L0101");
    let parsed = parse_text("enum E { V { u8 } }");
    assert_eq!(
        parsed.diagnostics[0].message,
        "a field of a variant written with braces is `name: Type`"
    );
}

/// Paths of any length in the three positions, and `u32::MAX`.
#[test]
fn paths_of_any_length_parse_in_types_expressions_and_patterns() {
    let expr = expression("a::b::c(u32::MAX, crate::d::E::F, super::g, self::h)");
    let ExprKind::Call { callee, arguments } = &expr.kind else {
        panic!()
    };
    assert_eq!(grouped(callee), "a::b::c");
    let texts: Vec<String> = arguments.iter().map(grouped).collect();
    assert_eq!(texts, ["u32::MAX", "crate::d::E::F", "super::g", "self::h"]);
    let parsed = parse_text(
        "fn f(e: crate::a::E) -> u8 { match e { crate::a::E::V(x) => x, a::E::W => 0, self::E::X { y } => y } }",
    );
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    // `self` alone is a value, `self::` a path, and `E::self` no name.
    let parsed = parse_text("fn f() -> u8 { E::self }");
    assert_eq!(parsed.diagnostics[0].code, "L0115");
    // The second `>` of `>>` closes the outer arguments, and `>=` gives
    // back its `=`.
    let parsed = parse_text("fn f() -> u8 { let x: Ghost<Option<u8>>= 1; x }");
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
}

/// Rust's precedence table (the Reference, "Expression precedence"), one
/// level against the next, with the associativity of each level. What is
/// valid in both languages groups the same way in both.
#[test]
fn operators_group_as_rusts_precedence_table_says() {
    let table: &[(&str, &str)] = &[
        // Method calls, fields, calls, and indexing bind tighter than unary.
        ("-a.b", "(-a.b)"),
        ("-f(a)", "(-f(a))"),
        ("-a.0", "(-a.0)"),
        ("!a.b", "(!a.b)"),
        ("!f(a).b", "(!f(a).b)"),
        ("-a.f(b).c", "(-a.f(b).c)"),
        ("-E::A", "(-E::A)"),
        // Unary operators nest, and bind tighter than `as`.
        ("- -a", "(-(-a))"),
        ("!!a", "(!(!a))"),
        ("-!a", "(-(!a))"),
        ("!-a", "(!(-a))"),
        ("-a as u8", "((-a) as u8)"),
        ("!a as u8", "((!a) as u8)"),
        ("-1", "(-1)"),
        ("-1u8", "(-1u8)"),
        // `as` is left to right and binds tighter than `*`.
        ("a as u8 as u16", "((a as u8) as u16)"),
        ("a as u8 * b", "((a as u8) * b)"),
        ("a * b as u8", "(a * (b as u8))"),
        ("a as u8 / b as u8", "((a as u8) / (b as u8))"),
        ("a as u8 + b as u8 * c", "((a as u8) + ((b as u8) * c))"),
        ("a as (u8, bool)", "(a as (u8, bool))"),
        ("a as ()", "(a as ())"),
        // `* / %`, left to right.
        ("a * b * c", "((a * b) * c)"),
        ("a / b % c", "((a / b) % c)"),
        ("a % b / c", "((a % b) / c)"),
        ("a * b / c % d", "(((a * b) / c) % d)"),
        // `* / %` bind tighter than `+ -`.
        ("a + b * c", "(a + (b * c))"),
        ("a * b + c", "((a * b) + c)"),
        ("a - b / c", "(a - (b / c))"),
        ("a % b - c", "((a % b) - c)"),
        ("a + b * c - d / e", "((a + (b * c)) - (d / e))"),
        ("1 + 2 * 3", "(1 + (2 * 3))"),
        // `+ -`, left to right.
        ("a - b - c", "((a - b) - c)"),
        ("a + b - c", "((a + b) - c)"),
        ("a - b + c", "((a - b) + c)"),
        ("a + b + c + d", "(((a + b) + c) + d)"),
        // Unary against `* / %` and `+ -`.
        ("-a * b", "((-a) * b)"),
        ("a * -b", "(a * (-b))"),
        ("a - -b", "(a - (-b))"),
        ("-a + b", "((-a) + b)"),
        ("!a + b", "((!a) + b)"),
        ("a.0 + b.1", "(a.0 + b.1)"),
        ("a.b(c) + d.e", "(a.b(c) + d.e)"),
        // `+ -` bind tighter than `<< >>`, which are left to right.
        ("a << b + c", "(a << (b + c))"),
        ("a + b >> c", "((a + b) >> c)"),
        ("a << b << c", "((a << b) << c)"),
        ("a >> b << c", "((a >> b) << c)"),
        ("a << b * c", "(a << (b * c))"),
        ("a as u8 << b", "((a as u8) << b)"),
        // `<< >>` bind tighter than `&`, which is left to right.
        ("a & b << c", "(a & (b << c))"),
        ("a << b & c", "((a << b) & c)"),
        ("a & b & c", "((a & b) & c)"),
        ("a & b + c", "(a & (b + c))"),
        // `&` binds tighter than `^`, which is left to right.
        ("a ^ b & c", "(a ^ (b & c))"),
        ("a & b ^ c", "((a & b) ^ c)"),
        ("a ^ b ^ c", "((a ^ b) ^ c)"),
        // `^` binds tighter than `|`, which is left to right.
        ("a | b ^ c", "(a | (b ^ c))"),
        ("a ^ b | c", "((a ^ b) | c)"),
        ("a | b | c", "((a | b) | c)"),
        ("a | b & c", "(a | (b & c))"),
        // Every operator above binds tighter than a comparison.
        ("a == b | c", "(a == (b | c))"),
        ("a & b == c", "((a & b) == c)"),
        ("a | b < c", "((a | b) < c)"),
        ("a ^ b != c", "((a ^ b) != c)"),
        ("a + b <= c", "((a + b) <= c)"),
        ("a < b + c", "(a < (b + c))"),
        ("a * b > c - d", "((a * b) > (c - d))"),
        ("a as u8 < b", "((a as u8) < b)"),
        ("a == b as u8", "(a == (b as u8))"),
        ("!a == b", "((!a) == b)"),
        ("-a >= b", "((-a) >= b)"),
        ("a << b >= c", "((a << b) >= c)"),
        // Comparisons bind tighter than `&&`, which is left to right.
        ("a == b && c != d", "((a == b) && (c != d))"),
        ("a && b == c", "(a && (b == c))"),
        ("a < b && b < c", "((a < b) && (b < c))"),
        ("a && b && c", "((a && b) && c)"),
        ("!a && b", "((!a) && b)"),
        // `&&` binds tighter than `||`, which is left to right.
        ("a || b && c", "(a || (b && c))"),
        ("a && b || c", "((a && b) || c)"),
        ("a || b || c", "((a || b) || c)"),
        ("a || b == c", "(a || (b == c))"),
        ("a | b || c", "((a | b) || c)"),
        ("a || b | c", "(a || (b | c))"),
        // Parentheses group.
        ("(a + b) * c", "((a + b) * c)"),
        ("a * (b + c)", "(a * (b + c))"),
        ("-(a + b)", "(-(a + b))"),
        ("(a as u8) as u8", "((a as u8) as u8)"),
        ("(-a) as u8", "((-a) as u8)"),
        ("-(a as u8)", "(-(a as u8))"),
        ("(a == b) == c", "((a == b) == c)"),
        ("a == (b == c)", "(a == (b == c))"),
        ("!(a && b)", "(!(a && b))"),
        ("a - (b - c)", "(a - (b - c))"),
        ("(a - b) - c", "((a - b) - c)"),
        ("a << (b << c)", "(a << (b << c))"),
        ("f(a + b, c * d)", "f((a + b), (c * d))"),
        ("(a + b).c", "(a + b).c"),
        ("(a, b + c)", "(a, (b + c),)"),
    ];
    for (text, expected) in table {
        assert_eq!(grouped(&expression(text)), *expected, "{text}");
    }
    // Inside a formula, `=>` is right to left and below `||`.
    let formulas: &[(&str, &str)] = &[
        ("a || b => c", "((a || b) => c)"),
        ("a => b || c", "(a => (b || c))"),
        ("a => b => c", "(a => (b => c))"),
        ("a && b => c || d", "((a && b) => (c || d))"),
        ("a + b <= c => d", "(((a + b) <= c) => d)"),
        ("a == b => -c < d", "((a == b) => ((-c) < d))"),
        ("a as u8 == b => c", "(((a as u8) == b) => c)"),
        ("!a => b", "((!a) => b)"),
        ("(a => b) => c", "((a => b) => c)"),
    ];
    for (text, expected) in formulas {
        assert_eq!(grouped(&formula(text)), *expected, "prop!({text})");
    }
}

#[test]
fn comparisons_reject_chaining_but_allow_explicit_grouping() {
    for text in [
        "a == b == c",
        "a < b < c",
        "a < b <= c",
        "a != b > c",
        "a == b < c",
        "a < b == c",
        "a + b < c < d",
        "prop!(a < b < c)",
    ] {
        let parsed = parse_text(&format!("fn f() -> bool {{ {text} }}"));
        let error = parsed
            .diagnostics
            .iter()
            .find(|error| error.code == "L0103")
            .unwrap_or_else(|| panic!("{text}: {:#?}", parsed.diagnostics));
        assert_eq!(error.message, "comparison operators cannot be chained");
        assert!(error.notes.iter().any(|note| note.contains("parenthesize")));
        // The second operator is reported, and the first is labelled.
        let spans: Vec<_> = error.labels.iter().map(|label| label.span).collect();
        assert_eq!(spans.len(), 2, "{text}");
        assert!(spans[1].start < spans[0].start, "{text}");
    }
    expression("(a == b) == c");
    expression("a == (b == c)");
    expression("a < b && b < c");
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
fn attributes_outside_the_closed_set_are_rejected_and_the_item_is_parsed() {
    for prefix in ["#[test]", "#![allow(unused)]"] {
        let parsed = parse_text(&format!("{prefix} fn f() -> u8 {{ 1 }}"));
        assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
        assert_eq!(parsed.diagnostics[0].code, "L0120");
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
    // Half the default thread stack, so the limit keeps a margin. To measure
    // the margin, `LOCUS_NESTING_STACK_KB=<n>` runs the test on a stack of
    // that size; the smallest that passes is the stack the parser needs.
    let stack_kb = std::env::var("LOCUS_NESTING_STACK_KB")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024usize);
    std::thread::Builder::new()
        .stack_size(stack_kb * 1024)
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
                // S5: Rust's loop forms, `return`, references, assignment,
                // and `let mut`, each nesting through its parts.
                ("while ", " { }"),
                ("while c { ", " }"),
                ("while let x = ", " { }"),
                ("while let Some(x) = ", " { }"),
                ("while let x = c { ", " }"),
                ("loop { ", " }"),
                ("loop { break ", " }"),
                ("loop { break; ", " }"),
                ("for x in ", " { }"),
                ("for x in 0..", " { }"),
                ("for x in 0..=", " { }"),
                ("for x in ", "..1 { }"),
                ("for x in xs { ", " }"),
                ("return ", ""),
                ("return (", ")"),
                ("&", ""),
                ("&mut ", ""),
                ("&(", ")"),
                ("&mut (", ")"),
                ("&&", ""),
                ("{ x = ", "; 1 }"),
                ("{ x.y = ", "; 1 }"),
                ("{ let mut x = ", "; 1 }"),
                ("{ let (mut a, b) = ", "; 1 }"),
                ("{ let mut x: u8 = ", "; 1 }"),
                ("{ if c { } else { } ", " }"),
                ("{ while c { } ", " }"),
                ("if c { 1 } else { 2 }.f(", ")"),
                // The operators of S3: prefix chains, `as` chains, and
                // binary nesting to either side through parentheses.
                ("-", ""),
                ("- (", ")"),
                ("!(", ")"),
                ("(", ") as u8"),
                ("((", ") as u8 as u8)"),
                ("1 + (", ")"),
                ("(", ") - 1"),
                ("(1 * ", ")"),
                ("1 / (", " % 2)"),
                ("1 << (", ")"),
                ("(", ") >> 1"),
                ("1 & (", ")"),
                ("1 ^ (", ")"),
                ("(", ") | 1"),
                ("1 == (", ")"),
                ("(", ") < 1"),
                ("1 && (", ")"),
                ("(", ") || 1"),
                ("prop!(p => -(", "))"),
                ("prop!(", " + 1 <= 2)"),
                ("-(1 + ", ")"),
                ("(-", " as u8)"),
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
            // A flat chain of one operator is bounded by the chain limit.
            for operator in [
                "+", "-", "*", "/", "%", "<<", ">>", "&", "^", "|", "&&", "||",
            ] {
                limit_reported(
                    format!(
                        "fn f() -> u8 {{ 1 {} }}",
                        format!("{operator} 1").repeat(512)
                    ),
                    operator,
                );
            }
            for postfix in ["as u8", ".f()", ".0"] {
                limit_reported(
                    format!("fn f() -> u8 {{ x {} }}", format!(" {postfix}").repeat(512)),
                    postfix,
                );
            }
            limit_reported(
                format!("fn f() -> Prop {{ prop!(1 {}) }}", "=> 1".repeat(512)),
                "=>",
            );
            for (open, close) in [
                ("(", ")"),
                ("(x: ", ",)"),
                ("(u8, ", ")"),
                ("fn(", ") -> u8"),
                ("fn() -> ", ""),
                ("fn(x: ", ") -> u8"),
                ("@(forall (h: ", ") { true })"),
                // S5: references in types.
                ("&", ""),
                ("&mut ", ""),
                ("&&", ""),
                ("&(", ")"),
                ("&mut (", ",)"),
                ("fn(&mut ", ") -> !"),
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
                // S4: variants with named fields, and paths of any length.
                ("E::V { a: ", " }"),
                ("E::V { a: _, b: ", ", .. }"),
                ("crate::a::E::V { a: ", " }"),
                // S5: a variant by one name, and `mut` on the innermost name.
                ("Some(", ")"),
                ("(mut a, ", ")"),
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
                // S5: the patterns of `while let` and `for`.
                limit_reported(
                    format!("fn f() -> u8 {{ while let {open}x{close} = 1 {{ }} 1 }}"),
                    &open,
                );
                limit_reported(
                    format!("fn f() -> u8 {{ for {open}x{close} in xs {{ }} 1 }}"),
                    &open,
                );
            }
            // S4: the same depth reached through an `impl` block and a method,
            // through `Self` and `self`, through type arguments, through a
            // named-field variant literal, and through the measure of a
            // `#[terminates]` attribute.
            for (before, open, close, after) in [
                (
                    "impl S { fn f(&self) -> Self { ",
                    "Self { x: ",
                    " }",
                    " } }",
                ),
                ("impl S { fn f(&self) -> u8 { ", "self.g(", ")", " } }"),
                ("impl S { fn f() -> u8 { ", "E::V { a: ", " }", " } }"),
                (
                    "impl S { fn f() -> u8 { ",
                    "E::V { a: 1, b: (",
                    ") }",
                    " } }",
                ),
                ("impl S { fn f(x: ", "Option<", ">", ") -> u8 { 1 } }"),
                ("impl S { fn f(x: ", "(u8, Ghost<", ">)", ") -> u8 { 1 } }"),
                (
                    "impl S { #[terminates(decreases = ",
                    "(",
                    ")",
                    ")] fn f() -> u8 { 1 } }",
                ),
                (
                    "#[terminates(decreases = ",
                    "f(",
                    ")",
                    ")] fn f() -> u8 { 1 }",
                ),
                (
                    "impl S { fn f() -> u8 { let ",
                    "E::V { a: ",
                    " }",
                    " = 1; 1 } }",
                ),
                (
                    "impl S { fn f() -> u8 { match y { ",
                    "E::V { a: ",
                    " }",
                    " => 1 } } }",
                ),
            ] {
                limit_reported(
                    format!("{before}{}1{}{after}", open.repeat(512), close.repeat(512)),
                    open,
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
         #[terminates] #[no_panic] #[no_io] fn same(x: u8, y: u8) -> Prop { prop!(x == y) }",
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
        "#[terminates] #[no_panic] #[no_io] fn prop(math: u8) -> u8 { let prop = math; prop }
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
    let text = "#[terminates] #[no_panic] #[no_io] fn same(x: u8, y: u8) -> Prop { [x == y] }
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
    // `#[condition]` reads as an attribute, which stands before an item only.
    for text in ["fn f() -> #[true] { _ }", "fn f() -> @(true) { #[true] }"] {
        let parsed = parse_text(text);
        let error = parsed
            .diagnostics
            .iter()
            .find(|d| d.code == "L0121")
            .unwrap();
        assert_eq!(
            error.message,
            "an attribute goes before an item, and nothing else takes one"
        );
        assert!(error.notes[0].contains("proof types use `@claim`"));
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
        "#[terminates] #[no_panic] #[no_io] fn good() -> u8 { 1 }",
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
fn math_fn_is_reported_once_with_a_fix_that_keeps_what_is_around_it() {
    // The retired keyword in every position it could be written: bare, after
    // attributes and visibility, and in an `impl` block. Each is reported
    // once, the declaration is still read, and the fix respells it as the
    // three promises in place.
    for (text, expected) in [
        (
            "math fn same(x: u8, y: u8) -> Prop { prop!(x == y) }",
            "#[terminates] #[no_panic] #[no_io] fn same(x: u8, y: u8) -> Prop { prop!(x == y) }",
        ),
        (
            "/// doc\n#[no_alloc]\npub(crate) math fn f(n: u8) -> u8 { n }",
            "/// doc\n#[no_alloc]\n#[terminates] #[no_panic] #[no_io] pub(crate) fn f(n: u8) -> u8 { n }",
        ),
        (
            "struct S { x: u8 } impl S { math fn get(self) -> u8 { self.x } }",
            "struct S { x: u8 } impl S { #[terminates] #[no_panic] #[no_io] fn get(self) -> u8 { self.x } }",
        ),
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:?}",
            parsed.diagnostics
        );
        let diagnostic = &parsed.diagnostics[0];
        assert_eq!(diagnostic.code, "L0114");
        assert_eq!(diagnostic.suggestions.len(), 1);
        assert_eq!(
            diagnostic.suggestions[0].applicability,
            Applicability::MachineApplicable
        );
        assert_eq!(
            parsed.program.declarations.len(),
            text.matches("struct").count() + 1
        );
        assert_eq!(fixed(text, &parsed), expected, "{text}");
        assert!(parse_text(expected).is_success(), "{expected}");
    }
    // Two of them are two diagnostics.
    let parsed = parse_text("math fn f() -> u8 { 1 } math fn g() -> u8 { 2 }");
    assert_eq!(parsed.diagnostics.len(), 2);
    assert_eq!(parsed.program.declarations.len(), 2);
    // `math` is a name everywhere else, before a `fn` type included.
    assert!(parse_text("fn math(math: u8) -> u8 { let math = math; math }").is_success());
    assert!(!parse_text("fn f(g: math fn(u8) -> u8) -> u8 { 1 }").is_success());
}

#[test]
fn recovery_keeps_functions_after_a_broken_function() {
    let parsed = parse_text(
        "fn broken() -> u8 { 1
         #[terminates] #[no_panic] #[no_io] fn good() -> Prop { prop!(true) }
         const claim: Prop = prop!(true);",
    );
    assert!(!parsed.is_success());
    assert_eq!(parsed.program.declarations.len(), 2, "{parsed:?}");
    assert!(matches!(
        parsed.program.declarations[0].kind,
        DeclarationKind::Function { .. }
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
        matches!(&arms[0].pattern.kind, PatternKind::Variant { path, arguments: None } if path.text() == "Event::Wrong")
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
    let PatternKind::Struct { path, fields, rest } = &arms[3].pattern.kind else {
        panic!()
    };
    assert_eq!(path.single().unwrap().text, "Lock");
    assert!(rest.is_none());
    assert!(fields[0].name.is_none());
    assert!(matches!(fields[0].pattern.kind, PatternKind::Name { .. }));
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
    assert!(matches!(&callee.kind, ExprKind::Path(path) if path.last().text == "Code"));
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
    let ExprKind::Struct { path, fields } =
        expression("Lock { failures: n.wrapping_add(1), open, }").kind
    else {
        panic!()
    };
    assert_eq!(path.single().unwrap().text, "Lock");
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
    assert!(matches!(result.unwrap().kind, TypeKind::Tuple(_)));
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
        ExprKind::Break(Some(value)) if matches!(value.kind, ExprKind::Tuple(_))
    ));
    let ExprKind::Block(else_block) = else_branch.kind else {
        panic!()
    };
    assert!(matches!(
        else_block.tail.unwrap().kind,
        ExprKind::Continue(Some(arguments)) if arguments.len() == 2
    ));
    // The state-passing form and Rust's are told apart by the result type,
    // and `break` and `continue` by whether they carry anything.
    assert!(matches!(
        expression("loop () -> u8 { continue() }").kind,
        ExprKind::Loop { state, result: Some(_), .. } if state.is_empty()
    ));
    assert!(matches!(
        expression("loop { break 1 }").kind,
        ExprKind::Loop { state, result: None, .. } if state.is_empty()
    ));
    for (text, alone) in [
        ("loop () -> u8 { break }", ExprKind::Break(None)),
        ("loop () -> u8 { continue }", ExprKind::Continue(None)),
    ] {
        let ExprKind::Loop { body, .. } = expression(text).kind else {
            panic!("{text}")
        };
        assert_eq!(body.tail.unwrap().kind, alone, "{text}");
    }
}

/// The bounds and kind of the range a `for` runs over.
fn range(iterable: &Expr) -> (&Expr, &Expr, RangeKind) {
    let ExprKind::Range { kind, lower, upper } = &iterable.kind else {
        panic!("not a range: {iterable:?}")
    };
    (lower, upper, *kind)
}

fn bound_name(pattern: &locus::ast::Pattern) -> (&str, bool) {
    let PatternKind::Name { name, mutable } = &pattern.kind else {
        panic!("not a name: {pattern:?}")
    };
    (&name.text, *mutable)
}

#[test]
fn a_for_header_separates_the_upper_bound_from_the_state_list() {
    let ExprKind::For {
        pattern,
        iterable,
        state,
        body,
    } = expression("for i in 0..n (acc: u8 = 0, same: @(acc == i) = _) { continue(acc, same) }")
        .kind
    else {
        panic!()
    };
    assert_eq!(bound_name(&pattern), ("i", false));
    let (lower, upper, kind) = range(&iterable);
    assert_eq!(kind, RangeKind::Exclusive);
    assert!(matches!(&lower.kind, ExprKind::Integer(literal) if literal.value == Natural::zero()));
    assert!(matches!(&upper.kind, ExprKind::Name(name) if name.text == "n"));
    assert_eq!(state.len(), 2);
    assert!(matches!(body.tail.unwrap().kind, ExprKind::Continue(_)));

    // Only the group directly before the body is the state list, and only
    // when it is empty or begins `name:`; `(a) {` closes a call.
    let ExprKind::For {
        iterable, state, ..
    } = expression("for i in start(a)..limit(a, b) () { continue() }").kind
    else {
        panic!()
    };
    let (lower, upper, _) = range(&iterable);
    assert!(matches!(lower.kind, ExprKind::Call { .. }));
    assert!(matches!(&upper.kind, ExprKind::Call { arguments, .. } if arguments.len() == 2));
    assert!(state.is_empty());
    let ExprKind::For {
        iterable, state, ..
    } = expression("for i in 0..limit(a) { continue }").kind
    else {
        panic!()
    };
    assert!(
        matches!(&range(&iterable).1.kind, ExprKind::Call { arguments, .. } if arguments.len() == 1)
    );
    assert!(state.is_empty());

    // Rust's form, without a state list, parses with no state.
    let ExprKind::For {
        state, iterable, ..
    } = expression("for i in 0..n { continue() }").kind
    else {
        panic!()
    };
    assert!(state.is_empty());
    assert!(matches!(&range(&iterable).1.kind, ExprKind::Name(name) if name.text == "n"));
}

// S5: `let mut`, assignment, `return`, Rust's loop forms, references, and
// the never type.

/// A pattern on one line: names with their `mut`, `_`, and tuples of those.
fn rendered_pattern(pattern: &locus::ast::Pattern) -> String {
    match &pattern.kind {
        PatternKind::Name { name, mutable } => {
            format!("{}{}", if *mutable { "mut " } else { "" }, name.text)
        }
        PatternKind::Wildcard => "_".into(),
        PatternKind::Tuple(parts) => format!(
            "({})",
            parts
                .iter()
                .map(rendered_pattern)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        PatternKind::Variant { path, arguments } => match arguments {
            None => path.text(),
            Some(arguments) => format!(
                "{}({})",
                path.text(),
                arguments
                    .iter()
                    .map(rendered_pattern)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        other => panic!("the table has no pattern like {other:?}"),
    }
}

/// A block's statements, each on one line, and its tail: `let`, `let mut`,
/// an assignment, or an expression, which ends in `;` whether or not the
/// source needed one.
fn rendered_statements(block: &Block) -> Vec<String> {
    let mut lines: Vec<String> = block
        .statements
        .iter()
        .map(|statement| match &statement.kind {
            StatementKind::Let {
                mutable,
                pattern,
                annotation,
                value,
            } => format!(
                "let {}{}{} = {};",
                if *mutable { "mut " } else { "" },
                // `let mut name` puts its `mut` on the name as well.
                match &pattern.kind {
                    PatternKind::Name {
                        name,
                        mutable: true,
                    } if *mutable => name.text.clone(),
                    _ if *mutable => panic!("`let mut` without a mutable name: {pattern:?}"),
                    _ => rendered_pattern(pattern),
                },
                annotation
                    .as_ref()
                    .map(|ty| format!(": {}", grouped_ty(ty)))
                    .unwrap_or_default(),
                grouped(value)
            ),
            StatementKind::Assign { place, value } => {
                format!("{} = {};", grouped(place), grouped(value))
            }
            StatementKind::Expression(expr) => format!("{};", grouped(expr)),
            StatementKind::Error => "<error>".into(),
        })
        .collect();
    lines.extend(block.tail.iter().map(|tail| grouped(tail)));
    lines
}

/// The first expression of a block: its first statement, or its tail, or
/// `()` when it is empty.
fn first_of(block: &Block) -> String {
    match (&block.statements[..], &block.tail) {
        ([statement, ..], _) => match &statement.kind {
            StatementKind::Expression(expr) => grouped(expr),
            other => panic!("{other:?}"),
        },
        ([], Some(tail)) => grouped(tail),
        ([], None) => "()".into(),
    }
}

/// The first expression of each branch of an `if`.
fn branches(expr: &Expr) -> (String, String) {
    let ExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &expr.kind
    else {
        panic!("not an `if`: {expr:?}")
    };
    let ExprKind::Block(else_block) = &else_branch.kind else {
        panic!()
    };
    (first_of(then_branch), first_of(else_block))
}

fn statement_expr(block: &Block, index: usize) -> &Expr {
    let StatementKind::Expression(expr) = &block.statements[index].kind else {
        panic!("statement {index} is not an expression")
    };
    expr
}

#[test]
fn let_mut_and_assignment_are_statements() {
    let block = body(
        "fn f(p: (u8, u8), s: S) -> u8 {
            let mut x = 1;
            let mut y: u8 = 2;
            let (mut a, b) = p;
            let (_, mut c) = p;
            x = x + 1;
            s.lock.failures = 0;
            p.0 = a;
            s.pairs.0.first = b;
            y = c;
            x
        }",
    );
    assert_eq!(
        rendered_statements(&block),
        [
            "let mut x = 1;",
            "let mut y: u8 = 2;",
            "let (mut a, b) = p;",
            "let (_, mut c) = p;",
            "x = (x + 1);",
            "s.lock.failures = 0;",
            "p.0 = a;",
            "s.pairs.0.first = b;",
            "y = c;",
            "x",
        ]
    );
    // `let mut name` is recorded on the statement and on the name alike;
    // `mut` inside a pattern is on the name alone.
    let StatementKind::Let {
        mutable, pattern, ..
    } = &block.statements[0].kind
    else {
        panic!()
    };
    assert!(*mutable);
    assert_eq!(bound_name(pattern), ("x", true));
    let StatementKind::Let {
        mutable, pattern, ..
    } = &block.statements[2].kind
    else {
        panic!()
    };
    assert!(!*mutable);
    let PatternKind::Tuple(parts) = &pattern.kind else {
        panic!()
    };
    assert_eq!(bound_name(&parts[0]), ("a", true));
    assert_eq!(bound_name(&parts[1]), ("b", false));
    // The value of an assignment is a whole expression, and the statement
    // spans from the place to the `;`.
    let StatementKind::Assign { place, value } = &block.statements[4].kind else {
        panic!()
    };
    assert!(matches!(place.kind, ExprKind::Name(_)));
    assert!(matches!(value.kind, ExprKind::Binary { .. }));
    let span = block.statements[4].span;
    assert_eq!(span.end - span.start, "x = x + 1;".len());
    // In a method, `self` and its fields are places.
    let parsed = parse_text("impl S { fn f(&mut self) -> u8 { self.count = 0; self.count } }");
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    // `mut` on a parameter parses and is kept.
    let parsed = parse_text("fn f(mut n: u8, m: u8) -> u8 { n = m; n }");
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    assert_eq!(
        rendered_item(&parsed.program.declarations[0]),
        "fn f(mut n: u8, m: u8) -> u8 { 2 statement(s) }"
    );
    let DeclarationKind::Function { parameters, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    assert!(parameters[0].mutable && !parameters[1].mutable);
    assert_eq!(parameters[0].span.start, "fn f(".len());
}

#[test]
fn an_assignment_needs_a_place_and_a_statement_of_its_own() {
    // L0123, in rustc's words, at the `=`, with the place labelled.
    for (text, place) in [
        ("f(x) = 1;", "f(x)"),
        ("x + 1 = 2;", "x + 1"),
        ("(x) = 1;", "(x)"),
        ("1 = x;", "1"),
        ("S { x } = s;", "S { x }"),
        ("x.f() = 1;", "x.f()"),
        ("x as u8 = 1;", "x as u8"),
        ("if c { a } else { b } = 1;", "if c { a } else { b }"),
    ] {
        let mut sources = SourceMap::default();
        let source = format!("fn f(x: u8) -> u8 {{ {text} x }}");
        let file = sources.add("test.lc", source.as_str());
        let source = sources.get(file);
        let parsed = parse(source);
        let first = &parsed.diagnostics[0];
        assert_eq!(first.code, "L0123", "{text}: {:#?}", parsed.diagnostics);
        assert_eq!(first.message, "invalid left-hand side of assignment");
        assert_eq!(source.slice(first.labels[0].span), Some("="), "{text}");
        assert_eq!(source.slice(first.labels[1].span), Some(place), "{text}");
        assert_eq!(first.labels[1].message, "cannot assign to this expression");
    }
    // L0124 where a value is needed: the message says what to write.
    for text in [
        "let y = x = 1;",
        "if x = 1 { 1 } else { 2 };",
        "while x = 1 { }",
        "f(x = 1);",
        "match x { _ => x = 1 };",
        "(x = 1);",
        "let y = { 1 } = 1;",
        "for i in x = 1 { }",
    ] {
        let parsed = parse_text(&format!("fn f(x: u8) -> u8 {{ {text} x }}"));
        let first = &parsed.diagnostics[0];
        assert_eq!(first.code, "L0124", "{text}: {:#?}", parsed.diagnostics);
        assert_eq!(first.message, "assignment is a statement and has no value");
        assert!(first.notes[0].contains("`==`"), "{text}");
    }
    // Compound assignment is still Rust's alone.
    let parsed = parse_text("fn f(x: u8) -> u8 { x += 1; x }");
    assert_eq!(parsed.diagnostics[0].code, "L0116");
    assert!(
        parsed.diagnostics[0]
            .message
            .starts_with("compound assignment")
    );
    // After a `;` is missing, the fix is the usual one.
    let parsed = parse_text("fn f(x: u8) -> u8 { x = 1 x }");
    assert_eq!(parsed.diagnostics[0].code, "L0102");
    assert_eq!(
        parsed.diagnostics[0].message,
        "expected `;` after this assignment"
    );
    // `=` after an operand is still the `=` of a state parameter.
    assert!(
        parse_text("fn f(n: u8) -> u8 { loop (ok: @within_limit(n) = _) -> u8 { break 1 } }")
            .is_success()
    );
}

#[test]
fn mut_goes_before_a_name_alone() {
    for (text, message) in [
        (
            "fn f(p: (u8, u8)) -> u8 { let mut (a, b) = p; a }",
            "`mut` must be attached to each individual binding",
        ),
        (
            "fn f(x: u8) -> u8 { let mut _ = x; x }",
            "`mut` must be attached to each individual binding",
        ),
        (
            "fn f(x: u8) -> u8 { let mut 1 = x; x }",
            "`mut` must be attached to each individual binding",
        ),
        (
            "fn f(x: u8) -> u8 { let mut mut y = x; x }",
            "`mut` on a binding may not be repeated",
        ),
        (
            "prop P(mut n: u8) { Any: @(true) }",
            "a parameter of a quantifier or a proposition never changes, and `mut` is not written on it",
        ),
        (
            "const c: Prop = prop!(forall (mut n: u8) { n == n });",
            "a parameter of a quantifier or a proposition never changes, and `mut` is not written on it",
        ),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let parsed = parse(source);
        let first = &parsed.diagnostics[0];
        assert_eq!(first.code, "L0125", "{text}: {:#?}", parsed.diagnostics);
        assert_eq!(first.message, message, "{text}");
        assert_eq!(source.slice(first.labels[0].span), Some("mut"), "{text}");
        assert!(first.notes[0].contains("`let (mut a, b) = pair;`"));
    }
    // Where no pattern follows, `mut` was meant as a name.
    for text in [
        "fn f() -> u8 { let mut = 1; 1 }",
        "fn f() -> u8 { let mut mut = 1; 1 }",
        "fn f() -> u8 { let (a, mut) = p; 1 }",
        "fn f(mut: u8) -> u8 { 1 }",
        "fn f(mut mut: u8) -> u8 { 1 }",
        "fn f() -> u8 { for mut in 0..1 { } 1 }",
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics[0].code, "L0115",
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert!(
            parsed.diagnostics[0]
                .message
                .starts_with("`mut` is a Rust keyword")
        );
    }
    // `mut` before an expression is not an expression.
    let parsed = parse_text("fn f(x: u8) -> u8 { let y = mut x; y }");
    assert_eq!(parsed.diagnostics[0].code, "L0100");
    assert_eq!(parsed.diagnostics[0].message, "expected an expression");
    // `mut S` is the binding `S`, as it is in Rust, so `let mut S { x }`
    // fails at the brace.
    let parsed = parse_text("fn f(s: S) -> u8 { let mut S { x } = s; x }");
    assert_eq!(parsed.diagnostics[0].code, "L0100");
    assert_eq!(parsed.diagnostics[0].message, "expected `=`, found `{`");
}

#[test]
fn return_break_and_continue_stand_alone_or_carry_a_value() {
    let block = body(
        "fn f(x: u8) -> u8 {
            if x == 0 { return; } else { }
            if x == 1 { return x } else { return (x, x); }
            loop { if x == 2 { break; } else { break x } }
            loop { if x == 3 { continue; } else { continue } }
            loop () -> u8 { if x == 4 { continue() } else { continue(x) } }
            return
        }",
    );
    assert_eq!(
        branches(statement_expr(&block, 0)),
        ("return".to_string(), "()".to_string())
    );
    assert_eq!(
        branches(statement_expr(&block, 1)),
        ("(return x)".to_string(), "(return (x, x,))".to_string())
    );
    let in_loop = |index: usize| -> (String, String) {
        let ExprKind::Loop { body, .. } = &statement_expr(&block, index).kind else {
            panic!("statement {index} is not a loop")
        };
        branches(body.tail.as_ref().unwrap())
    };
    assert_eq!(in_loop(2), ("break".to_string(), "(break x)".to_string()));
    assert_eq!(in_loop(3), ("continue".to_string(), "continue".to_string()));
    assert_eq!(
        in_loop(4),
        ("continue()".to_string(), "continue(x)".to_string())
    );
    assert_eq!(grouped(block.tail.as_ref().unwrap()), "return");
    // The value of a jump is a whole expression, and a jump is an operand.
    assert_eq!(grouped(&expression("return a + b")), "(return (a + b))");
    assert_eq!(
        grouped(&expression("(return, break, continue)")),
        "(return, break, continue,)"
    );
    assert_eq!(
        grouped(&expression("f(return 1, break)")),
        "f((return 1), break)"
    );
    assert_eq!(grouped(&expression("break (1, 2).0")), "(break (1, 2,).0)");
    // A `return` or `break` alone is not an operand of anything.
    assert!(!parse_text("fn f() -> u8 { return + 1 }").is_success());
    assert!(!parse_text("fn f() -> u8 { return as u8 }").is_success());
}

#[test]
fn loops_take_rusts_forms_and_the_state_passing_one() {
    let block = body(
        "fn f(n: u8, items: List, pairs: List) -> u8 {
            loop { break; }
            while n < 3 { n = n + 1; }
            while let Some(v) = items.next() { n = v; }
            for i in 0..n { n = i; }
            for i in 0..=n { n = i; }
            for (a, b) in pairs { n = a; }
            for x in items { n = x; }
            for x in items.iter() { n = x; }
            for E::V(x) in items { n = x; }
            for i in 0..n (s: u8 = 0) { continue(s) }
            loop (s: u8 = 0) -> u8 { break s }
            n
        }",
    );
    assert_eq!(
        rendered_statements(&block),
        [
            "<loop>;",
            "<while>;",
            "<while let>;",
            "<for>;",
            "<for>;",
            "<for>;",
            "<for>;",
            "<for>;",
            "<for>;",
            "<for>;",
            "<loop (state)>;",
            "n",
        ]
    );
    let for_header = |index: usize| -> (String, String, usize) {
        let ExprKind::For {
            pattern,
            iterable,
            state,
            ..
        } = &statement_expr(&block, index).kind
        else {
            panic!("statement {index} is not a `for`")
        };
        (rendered_pattern(pattern), grouped(iterable), state.len())
    };
    assert_eq!(for_header(3), ("i".into(), "(0..n)".into(), 0));
    assert_eq!(for_header(4), ("i".into(), "(0..=n)".into(), 0));
    assert_eq!(for_header(5), ("(a, b)".into(), "pairs".into(), 0));
    assert_eq!(for_header(6), ("x".into(), "items".into(), 0));
    assert_eq!(for_header(7), ("x".into(), "items.iter()".into(), 0));
    assert_eq!(for_header(8), ("E::V(x)".into(), "items".into(), 0));
    assert_eq!(for_header(9), ("i".into(), "(0..n)".into(), 1));
    let ExprKind::While {
        pattern,
        condition,
        body,
    } = &statement_expr(&block, 2).kind
    else {
        panic!()
    };
    assert_eq!(rendered_pattern(pattern.as_ref().unwrap()), "Some(v)");
    assert_eq!(grouped(condition), "items.next()");
    assert_eq!(rendered_statements(body), ["n = v;"]);
    let ExprKind::While {
        pattern, condition, ..
    } = &statement_expr(&block, 1).kind
    else {
        panic!()
    };
    assert!(pattern.is_none());
    assert_eq!(grouped(condition), "(n < 3)");
    // The bounds of a range are whole expressions, and the range is the
    // iterable of the loop, not an operator anywhere else.
    let ExprKind::For { iterable, .. } = expression("for i in a + 1..b * 2 { }").kind else {
        panic!()
    };
    assert_eq!(grouped(&iterable), "((a + 1)..(b * 2))");
    for text in ["0..n", "let r = 0..n; 1", "f(0..n)"] {
        let parsed = parse_text(&format!("fn f(n: u8) -> u8 {{ {text} }}"));
        assert!(!parsed.is_success(), "{text}");
    }
    // A `while let` takes any pattern and needs its `=`.
    assert!(parse_text("fn f() -> u8 { while let (a, _) = p { } 1 }").is_success());
    let parsed = parse_text("fn f() -> u8 { while let (x, _) { } 1 }");
    assert_eq!(parsed.diagnostics[0].message, "expected `=`, found `{`");
}

#[test]
fn a_struct_literal_cannot_stand_in_a_loop_header() {
    // `while flag {` and `for x in items {` begin the body at `{`.
    let ExprKind::While { condition, .. } = expression("while flag { flag = false; }").kind else {
        panic!()
    };
    assert!(matches!(condition.kind, ExprKind::Name(_)));
    let ExprKind::While { condition, .. } =
        expression("while let Some(x) = queue { queue = x; }").kind
    else {
        panic!()
    };
    assert!(matches!(condition.kind, ExprKind::Name(_)));
    let ExprKind::For { iterable, .. } = expression("for x in items { }").kind else {
        panic!()
    };
    assert!(matches!(iterable.kind, ExprKind::Name(_)));
    let ExprKind::For { iterable, .. } = expression("for x in lo..items { }").kind else {
        panic!()
    };
    assert!(matches!(range(&iterable).1.kind, ExprKind::Name(_)));
    // Delimiters lift the restriction, as for `if`.
    let ExprKind::While { condition, .. } =
        expression("while same(Lock { failures: 0, open }) { }").kind
    else {
        panic!()
    };
    assert!(matches!(condition.kind, ExprKind::Call { .. }));
}

#[test]
fn an_expression_ending_in_a_block_is_a_whole_statement() {
    // As in Rust, `if c { } else { } (a, b)` is an `if` statement and then
    // a tuple, not a call; the same for `match`, the loops, and a block.
    let block = body(
        "fn f(c: bool, a: u8, b: u8) -> (u8, u8) {
            if c { g(); } else { h(); }
            match c { _ => 1 }
            loop { break; }
            while c { }
            for i in 0..a { }
            { a }
            if c { a } else { b }.wrapping_add(1);
            match c { _ => a }.f().g();
            (a, b)
        }",
    );
    assert_eq!(
        rendered_statements(&block),
        [
            "<if>;",
            "<match>;",
            "<loop>;",
            "<while>;",
            "<for>;",
            "<block>;",
            "<if>.wrapping_add(1);",
            "<match>.f().g();",
            "(a, b,)",
        ]
    );
    // A `;` after such a statement is allowed, and none is needed before
    // the tail.
    let block = body("fn f(c: bool) -> u8 { if c { 1 } else { 2 }; while c { }; 3 }");
    assert_eq!(rendered_statements(&block), ["<if>;", "<while>;", "3"]);
    let block = body("fn f(c: bool) -> u8 { { 1 } if c { 1 } else { 2 } }");
    assert_eq!(rendered_statements(&block), ["<block>;", "<if>"]);
    // An operator after such a statement begins the next statement, where
    // it is no expression; the value of a `let` is not restricted.
    for text in ["match c { _ => 1 } + 1", "if c { 1 } else { 2 } == 3"] {
        let parsed = parse_text(&format!("fn f(c: bool) -> u8 {{ {text} }}"));
        assert_eq!(
            parsed.diagnostics[0].message, "expected an expression",
            "{text}"
        );
    }
    let block = body("fn f(c: bool) -> u8 { let x = if c { 1 } else { 2 } + 1; x }");
    assert_eq!(rendered_statements(&block), ["let x = (<if> + 1);", "x"]);
    // The state-passing loops are statements in the same way.
    let block = body("fn f(n: u8) -> (u8, u8) { for i in 0..n () { continue() } (n, n) }");
    assert_eq!(rendered_statements(&block), ["<for>;", "(n, n,)"]);
}

#[test]
fn references_and_the_never_type_parse_in_types_and_expressions() {
    for (text, expected) in [
        ("&x", "(&x)"),
        ("&mut x", "(&mut x)"),
        ("&mut lock.failures", "(&mut lock.failures)"),
        ("&pair.0", "(&pair.0)"),
        ("f(&mut a, &b, c)", "f((&mut a), (&b), c)"),
        ("&&x", "(&(&x))"),
        ("&&mut x", "(&(&mut x))"),
        ("&x as u8", "((&x) as u8)"),
        ("&x + 1", "((&x) + 1)"),
        ("&f(x).y", "(&f(x).y)"),
        ("-&x", "(-(&x))"),
        ("!&x", "(!(&x))"),
        ("&(a + b)", "(&(a + b))"),
        ("a & &b", "(a & (&b))"),
        ("a && &b", "(a && (&b))"),
    ] {
        assert_eq!(grouped(&expression(text)), expected, "{text}");
    }
    let parsed = parse_text(
        "fn f(a: &u8, b: &mut Lock, c: &&u8, d: &mut (u8, u8), e: &Option<u8>, g: &mut &u8) -> ! { loop { } }",
    );
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    assert_eq!(
        rendered_item(&parsed.program.declarations[0]),
        "fn f(a: &u8, b: &mut Lock, c: &&u8, d: &mut (u8, u8), e: &Option<u8>, g: &mut &u8) -> ! { 1 statement(s) }"
    );
    let parsed = parse_text("impl S { fn f(&mut self, other: &S) -> &mut S { self } }");
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    // A reference is not a place, and `&mut x = y` is an invalid one.
    let parsed = parse_text("fn f(x: u8, y: u8) -> u8 { &mut x = y; x }");
    assert_eq!(parsed.diagnostics[0].code, "L0123");
    // Dereference stays Rust's alone, in types and in expressions.
    for text in [
        "fn f(x: *const u8) -> u8 { 1 }",
        "fn f(x: &u8) -> u8 { *x }",
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics[0].code, "L0116",
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert!(parsed.diagnostics[0].message.starts_with("dereferences"));
    }
    // `!` is a type, and `!` before a value is still negation.
    let block = body("fn f(x: bool) -> ! { let y: ! = loop { }; !x; y }");
    assert_eq!(
        rendered_statements(&block),
        ["let y: ! = <loop>;", "(!x);", "y"]
    );
    // The spans of a reference and of a `&&` reference are exact.
    let ExprKind::Ref { expr, .. } = expression("&&x").kind else {
        panic!()
    };
    let inner = expr.span;
    assert_eq!(
        (inner.start, inner.end),
        (
            "fn example() -> u8 { &".len(),
            "fn example() -> u8 { &&x".len()
        )
    );
}

#[test]
fn function_types_record_their_parameter_names() {
    let parsed = parse_text(
        "#[terminates] #[no_panic] #[no_io] fn apply(f: fn(x: u8) -> @(x == x), g: fn(u8, bool) -> u8) -> () { () }",
    );
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let DeclarationKind::Function { parameters, .. } = &parsed.program.declarations[0].kind else {
        panic!()
    };
    let TypeKind::Function {
        parameters: inputs,
        result,
    } = &parameters[0].ty.kind
    else {
        panic!()
    };
    assert_eq!(inputs[0].name.as_ref().unwrap().text, "x");
    assert!(matches!(result.kind, TypeKind::Proof(_)));
    assert!(matches!(
        &parameters[1].ty.kind,
        TypeKind::Function { parameters, .. } if parameters.len() == 2
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
        // The index of a `for` is a pattern, like a binding.
        (
            "a loop index",
            "fn f() -> u8 { for {} in 0..1 () { continue() } }",
            &["true", "false"],
        ),
        (
            "a `while let` pattern",
            "fn f() -> u8 { while let {} = e { } 1 }",
            &["true", "false"],
        ),
        ("a `mut` binding", "fn f() -> u8 { let mut {} = 1; 1 }", &[]),
        // `mut self` is a receiver, reported as one where no method is.
        (
            "a `mut` parameter",
            "fn f(mut {}: u8) -> u8 { 1 }",
            &["self"],
        ),
        ("a field of a place", "fn f() -> u8 { s.{} = 1; 1 }", &[]),
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
            // A `self` parameter and `Self` are Locus outside their place,
            // an `impl` block; every other keyword in every other position
            // is no name.
            let code = match (*keyword, *position) {
                ("self", "a parameter" | "a bound variable") => "L0100",
                (
                    "Self",
                    "a type"
                    | "a binding"
                    | "a binding in a tuple"
                    | "a binding in an arm"
                    | "a loop index"
                    | "a `while let` pattern",
                ) => "L0100",
                _ => "L0115",
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
                .filter(|diagnostic| diagnostic.labels[0].span == first.labels[0].span)
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
    // The binary operators of Rust's table all parse (S3); before an
    // operand, `&`, `|`, and `*` begin what Locus does not have yet.
    for (text, code, message) in [
        (
            "|y| y",
            "L0116",
            "closures and or-patterns (`|`) are not in Locus yet",
        ),
        ("f(x)?", "L0116", "the `?` operator is not in Locus yet"),
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
            "0..=9",
            "L0116",
            "inclusive ranges (`..=`) are not in Locus yet, except in the header of a `for`",
        ),
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
    assert!(parsed.diagnostics[0].message.starts_with("dereferences"));
}

#[test]
fn constructs_of_rust_are_reported_as_not_in_locus_yet() {
    for (text, message, declarations) in [
        (
            "impl Show for S { fn get() -> u8 { 1 } }",
            "traits (`impl Trait for Type`) are not in Locus yet",
            0,
        ),
        (
            "impl<T> S { fn get() -> u8 { 1 } }",
            "generic parameters are not in Locus yet",
            0,
        ),
        (
            "impl S<T> { fn get() -> u8 { 1 } }",
            "generic parameters are not in Locus yet",
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
            "unsafe fn f() -> u8 { 1 }",
            "`unsafe` is not in Locus yet",
            1,
        ),
        ("async fn f() -> u8 { 1 }", "`async` is not in Locus yet", 1),
        (
            "fn f(x: dyn T) -> u8 { 1 }",
            "`dyn` trait objects are not in Locus yet",
            0,
        ),
        (
            "fn f(x: impl T) -> u8 { 1 }",
            "`impl Trait` types are not in Locus yet",
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
            "fn f(x: u8) -> u8 { unsafe { x } }",
            "`unsafe` is not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { let ref y = x; x }",
            "`ref` bindings are not in Locus yet",
            1,
        ),
        (
            "fn f(x: u8) -> u8 { move || x; x }",
            "closures (`move`) are not in Locus yet",
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
fn an_unknown_or_misplaced_attribute_is_reported_once_and_its_item_is_parsed() {
    // Unknown names: L0120, which lists the closed set.
    for (text, declarations) in [
        ("#[serde(rename_all = \"x\")] struct S { x: u8 }", 1),
        ("#![allow(unused)] fn f() -> u8 { 1 }", 1),
        ("#[test] #[cfg(any(a, b))] fn f() -> u8 { 1 }", 1),
        ("fn f() -> u8 { 1 } #[trailing]", 1),
        ("struct S { #[serde(rename = \"y\")] x: u8 }", 1),
        ("#[never closed fn f() -> u8 { 1 }", 0),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let parsed = parse(source);
        let unknown: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "L0120")
            .collect();
        let others = parsed.diagnostics.len() - unknown.len();
        assert!(!unknown.is_empty(), "{text}: {:#?}", parsed.diagnostics);
        assert!(others <= 1, "{text}: {:#?}", parsed.diagnostics);
        assert_eq!(parsed.program.declarations.len(), declarations, "{text}");
        for diagnostic in unknown {
            assert!(diagnostic.message.ends_with("is not an attribute of Locus"));
            assert_eq!(
                diagnostic.notes[0],
                "the attributes are `#[terminates]`, `#[terminates(decreases = e)]`, `#[no_panic]`, `#[no_alloc]`, `#[no_io]`, and `#[derive(...)]`; Locus has no user-defined attributes"
            );
            let covered = source.slice(diagnostic.labels[0].span).unwrap();
            assert!(
                text.contains(&format!("#[{covered}")) || text.contains(&format!("#![{covered}"))
            );
        }
    }
    // A known attribute where no item begins: L0121, once, brackets and all.
    for (text, declarations) in [
        ("fn f() -> u8 { #[no_panic] let x = 1; x }", 1),
        ("fn f() -> u8 { #[no_panic] 1 }", 1),
        ("fn f(x: #[no_panic] u8) -> u8 { 1 }", 0),
        ("fn f() -> u8 { let #[no_panic] x = 1; x }", 1),
    ] {
        let mut sources = SourceMap::default();
        let file = sources.add("test.lc", text);
        let source = sources.get(file);
        let parsed = parse(source);
        let misplaced: Vec<_> = parsed
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "L0121")
            .collect();
        assert_eq!(misplaced.len(), 1, "{text}: {:#?}", parsed.diagnostics);
        assert_eq!(
            source.slice(misplaced[0].labels[0].span),
            Some("#[no_panic]")
        );
        assert_eq!(parsed.program.declarations.len(), declarations, "{text}");
    }
    let parsed = parse_text("struct S { #[no_panic] x: u8 }");
    assert_eq!(parsed.diagnostics.len(), 1);
    assert_eq!(parsed.diagnostics[0].code, "L0121");
    assert_eq!(
        parsed.diagnostics[0].message,
        "`#[no_panic]` goes before an item; a field or a variant takes no attribute"
    );
    assert_eq!(parsed.program.declarations.len(), 1);
}

#[test]
fn attributes_of_the_closed_set_are_kept_on_the_item_in_every_shape() {
    use locus::ast::AttributeKind;
    let text = "\
#![no_panic]
#![terminates]
//! about the file
#![no_io]

/// about f
#[terminates(decreases = n - 1)]
#[no_alloc] #[derive(Clone, std::marker::Copy)]
#[derive()]
/// more about f
pub fn f(n: u8) -> u8 { n }
";
    let parsed = parse_text(text);
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let program = parsed.program;
    assert_eq!(
        program
            .doc
            .iter()
            .map(|doc| doc.text.as_str())
            .collect::<Vec<_>>(),
        [" about the file"]
    );
    let kinds: Vec<_> = program
        .attributes
        .iter()
        .map(|attribute| attribute.kind.name())
        .collect();
    assert_eq!(kinds, ["no_panic", "terminates", "no_io"]);
    assert_eq!(program.declarations.len(), 1);
    let declaration = &program.declarations[0];
    assert_eq!(
        declaration
            .doc
            .iter()
            .map(|doc| doc.text.as_str())
            .collect::<Vec<_>>(),
        [" about f", " more about f"]
    );
    let attributes = &declaration.attributes;
    assert_eq!(attributes.len(), 4);
    let AttributeKind::Terminates {
        decreases: Some(measure),
    } = &attributes[0].kind
    else {
        panic!("{:?}", attributes[0]);
    };
    assert_eq!(grouped(measure), "(n - 1)");
    assert!(matches!(attributes[1].kind, AttributeKind::NoAlloc));
    let AttributeKind::Derive(traits) = &attributes[2].kind else {
        panic!("{:?}", attributes[2]);
    };
    assert_eq!(
        traits
            .iter()
            .map(locus::ast::Path::text)
            .collect::<Vec<_>>(),
        ["Clone", "std::marker::Copy"]
    );
    assert!(matches!(&attributes[3].kind, AttributeKind::Derive(traits) if traits.is_empty()));
    // The item's span covers its doc comments and attributes.
    assert_eq!(declaration.span.start, text.find("/// about f").unwrap());
    assert_eq!(declaration.span.end, text.len() - 1);
    // The bare and the measured form of `terminates` are distinct.
    let parsed = parse_text("#[terminates] fn f() -> u8 { 1 }");
    assert!(matches!(
        parsed.program.declarations[0].attributes[0].kind,
        AttributeKind::Terminates { decreases: None }
    ));
}

#[test]
fn an_attribute_in_the_wrong_shape_or_place_says_what_is_expected() {
    for (text, message) in [
        (
            "#[no_panic(x)] fn f() -> u8 { 1 }",
            "`#[no_panic]` takes no arguments",
        ),
        (
            "#[no_io()] fn f() -> u8 { 1 }",
            "`#[no_io]` takes no arguments",
        ),
        (
            "#[derive] struct S { x: u8 }",
            "`#[derive]` takes a list of traits in parentheses, as in `#[derive(Clone, Copy)]`",
        ),
        (
            "#[derive(1)] struct S { x: u8 }",
            "`#[derive]` lists traits by name, as in `#[derive(Clone, Copy)]`",
        ),
        (
            "#[terminates(foo = 1)] fn f() -> u8 { 1 }",
            "`#[terminates]` takes `decreases = expression` and nothing else",
        ),
        (
            "#[terminates(decreases)] fn f() -> u8 { 1 }",
            "`#[terminates]` takes `decreases = expression` and nothing else",
        ),
        (
            "#![derive(Clone)] fn f() -> u8 { 1 }",
            "`derive` goes on a struct or an enum, not at the top of the file",
        ),
        (
            "#![terminates(decreases = n)] fn f() -> u8 { 1 }",
            "`decreases` names the measure of one function; at the top of the file write `#![terminates]`",
        ),
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].code, "L0121", "{text}");
        assert_eq!(parsed.diagnostics[0].message, message, "{text}");
        // The item after a bad attribute is still read.
        assert_eq!(parsed.program.declarations.len(), 1, "{text}");
    }
    // Inner forms after the first item are reported and left out, and both
    // items are read.
    for (text, message) in [
        (
            "fn g() -> u8 { 1 } #![no_panic] fn f() -> u8 { 1 }",
            "an inner attribute (`#![...]`) goes at the top of the file, before any item",
        ),
        (
            "fn g() -> u8 { 1 } //! late\nfn f() -> u8 { 1 }",
            "an inner doc comment (`//!`) goes at the top of the file, before any item",
        ),
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].code, "L0121", "{text}");
        assert_eq!(parsed.diagnostics[0].message, message, "{text}");
        assert_eq!(parsed.program.declarations.len(), 2, "{text}");
        assert!(parsed.program.declarations[1].attributes.is_empty());
    }
    // A shape error inside the parentheses reports the expression's error.
    let parsed = parse_text("#[terminates(decreases = )] fn f() -> u8 { 1 }");
    assert_eq!(parsed.diagnostics.len(), 1, "{:#?}", parsed.diagnostics);
    assert_eq!(parsed.diagnostics[0].code, "L0100");
    assert_eq!(parsed.program.declarations.len(), 1);
    // An attribute with nothing after it.
    let parsed = parse_text("#[no_panic]");
    assert_eq!(parsed.diagnostics.len(), 1);
    assert!(
        parsed.diagnostics[0]
            .message
            .starts_with("expected a declaration")
    );
}

#[test]
fn doc_comments_are_lexed_as_tokens_and_kept_where_items_begin() {
    let lexed = lex_text("/// a\n//! b\n//// not\n/** c */ /*! d */ /**/ /*** e */ x");
    assert_eq!(
        kinds(&lexed),
        [
            K::OuterDoc,
            K::InnerDoc,
            K::OuterDoc,
            K::InnerDoc,
            K::Name,
            K::Eof
        ]
    );
    let texts: Vec<_> = lexed.tokens[..4]
        .iter()
        .map(|token| match lexed.literal(*token) {
            Some(Literal::String(text)) => text.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(texts, [" a", " b", " c ", " d "]);

    let parsed = parse_text(
        "/// the struct\nstruct S {\n    /// the field\n    pub x: u8,\n}\n/// the enum\nenum E {\n    /// a variant\n    A,\n}\nprop P {\n    /// a constructor\n    Q: @(true),\n}\n/** block */\nimpl S {\n    /// a method\n    fn f(&self) -> u8 { 1 }\n}\n",
    );
    assert!(parsed.is_success(), "{:#?}", parsed.diagnostics);
    let declarations = &parsed.program.declarations;
    assert_eq!(declarations[0].doc[0].text, " the struct");
    let DeclarationKind::Struct { fields, .. } = &declarations[0].kind else {
        panic!()
    };
    assert_eq!(fields[0].doc[0].text, " the field");
    let DeclarationKind::Enum { variants, .. } = &declarations[1].kind else {
        panic!()
    };
    assert_eq!(variants[0].doc[0].text, " a variant");
    let DeclarationKind::Prop { variants, .. } = &declarations[2].kind else {
        panic!()
    };
    assert_eq!(variants[0].doc[0].text, " a constructor");
    assert_eq!(declarations[3].doc[0].text, " block ");
    let DeclarationKind::Impl { methods, .. } = &declarations[3].kind else {
        panic!()
    };
    assert_eq!(methods[0].doc[0].text, " a method");

    // Anywhere else, a doc comment is reported where it stands.
    for text in [
        "fn f() -> u8 { /// here\n 1 }",
        "fn f() -> u8 { let x = /// here\n 1; x }",
        "fn f(x: /// here\n u8) -> u8 { 1 }",
    ] {
        let parsed = parse_text(text);
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "{text}: {:#?}",
            parsed.diagnostics
        );
        assert_eq!(parsed.diagnostics[0].code, "L0100", "{text}");
        assert!(
            parsed.diagnostics[0].message.contains("doc comment"),
            "{text}: {}",
            parsed.diagnostics[0].message
        );
    }
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
