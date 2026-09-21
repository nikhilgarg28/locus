//! Recursive-descent syntax parser with Pratt expression precedence.

use crate::ast::*;
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::lexer::{Literal, Token, TokenKind as K, lex};
use crate::source::{SourceFile, Span};

const MAX_DEPTH: usize = 64;
const MAX_EXPRESSION_CHAIN: usize = 128;
type ParseResult<T> = Result<T, ()>;

#[derive(Debug)]
pub struct Parsed {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
    pub stats: ParseStats,
}

/// The work one parse did, for the tests of the progress guarantee: `steps`
/// stays within a constant multiple of `tokens`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseStats {
    /// Tokens the lexer produced, the final end-of-file token included.
    pub tokens: usize,
    /// One for every iteration of a parser loop and one for every production
    /// entered through the depth guard.
    pub steps: usize,
}

impl Parsed {
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn parse(source: &SourceFile) -> Parsed {
    let lexed = lex(source);
    Parser {
        source,
        tokens: lexed.tokens,
        literals: lexed.literals,
        position: 0,
        depth: 0,
        steps: 0,
        no_struct: false,
        for_header: false,
        closers: None,
        diagnostics: lexed.diagnostics,
    }
    .program()
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    /// The values of the integer and string tokens, by `Token::literal`.
    literals: Vec<Literal>,
    position: usize,
    depth: usize,
    steps: usize,
    /// Set in the header of an `if`, `match`, or `for`, where `Name {` begins
    /// the following block rather than a struct literal.
    no_struct: bool,
    /// Set in the bounds of a `for`, where `( ... ) {` is the state list.
    for_header: bool,
    /// For each opening delimiter, the token that closes it; built by the
    /// first `for` header that asks, so that the lookahead is not a rescan.
    closers: Option<Vec<Option<usize>>>,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn program(mut self) -> Parsed {
        let mut program = Program::default();
        while !self.at(K::Eof) {
            self.step();
            if self.eat(K::Error).is_some() {
                continue;
            }
            let start = self.position;
            match self.declaration() {
                Ok(declaration) => program.declarations.push(declaration),
                Err(()) => self.recover_declaration(),
            }
            if self.position == start {
                self.bump();
            }
        }
        self.diagnostics
            .sort_by_key(|diagnostic| diagnostic.labels[0].span.start);
        Parsed {
            program,
            diagnostics: self.diagnostics,
            stats: ParseStats {
                tokens: self.tokens.len(),
                steps: self.steps,
            },
        }
    }

    /// The progress guarantee, made countable. Every loop below calls this
    /// once per iteration, and `nested` once per production it admits. The
    /// position never moves back, an iteration consumes a token or is the last
    /// of its loop, and a production consumes a token or fails, so the count
    /// stays within a constant multiple of the number of tokens.
    fn step(&mut self) {
        self.steps += 1;
    }

    fn current(&self) -> Token {
        self.tokens[self.position]
    }

    fn peek(&self, distance: usize) -> K {
        self.tokens
            .get(self.position + distance)
            .map_or(K::Eof, |token| token.kind)
    }

    fn at(&self, kind: K) -> bool {
        self.current().kind == kind
    }

    fn bump(&mut self) -> Token {
        let token = self.current();
        if token.kind != K::Eof {
            self.position += 1;
        }
        token
    }

    fn eat(&mut self, kind: K) -> Option<Token> {
        self.at(kind).then(|| self.bump())
    }

    /// An error token was reported by the lexer, and a token of Rust that
    /// Locus does not use yet is reported as that, whatever was expected.
    fn fail<T>(&mut self, message: impl Into<String>) -> ParseResult<T> {
        if !self.at(K::Error) && !self.rust_only() {
            self.diagnostics
                .push(Diagnostic::error("L0100", message, self.current().span));
        }
        Err(())
    }

    fn expect(&mut self, kind: K) -> ParseResult<Token> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            self.fail(format!(
                "expected {}, found {}",
                kind.description(),
                self.current().kind.description()
            ))
        }
    }

    fn close(&mut self, kind: K, opening: Token) -> ParseResult<Token> {
        if self.at(kind) {
            return Ok(self.bump());
        }
        if !self.at(K::Error) {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0101",
                    format!("expected {} to close this delimiter", kind.description()),
                    self.current().span,
                )
                .label(opening.span, "delimiter opened here"),
            );
        }
        Err(())
    }

    fn nested<T>(&mut self, operation: impl FnOnce(&mut Self) -> ParseResult<T>) -> ParseResult<T> {
        if self.depth >= MAX_DEPTH {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0108",
                    "syntax nesting exceeds the parser limit",
                    self.current().span,
                )
                .note("split deeply nested expressions or types into smaller definitions"),
            );
            return Err(());
        }
        self.step();
        self.depth += 1;
        let result = operation(self);
        self.depth -= 1;
        result
    }

    /// Reports the current token if it belongs to Rust and not yet to Locus:
    /// a keyword Locus does not use, an attribute, or an operator.
    #[inline(never)]
    fn rust_only(&mut self) -> bool {
        let token = self.current();
        if self.attribute_start() {
            self.attribute();
            return true;
        }
        let spelling = self.source.slice(token.span).unwrap_or_default();
        let diagnostic = if token.kind == K::Keyword {
            match keyword_construct(spelling) {
                Some((message, note)) => {
                    let diagnostic = Diagnostic::error("L0116", message, token.span);
                    match note {
                        Some(note) => diagnostic.note(note),
                        None => diagnostic,
                    }
                }
                None => keyword_is_no_name(spelling, token.span),
            }
        } else {
            let Some(message) = rust_only_token(token.kind, spelling) else {
                return false;
            };
            Diagnostic::error("L0116", message, token.span)
        };
        self.diagnostics.push(diagnostic);
        true
    }

    /// A keyword where a name belongs. Before a name, `mut` or `pub` is Rust
    /// that Locus does not have yet, and so are the keywords that stand for
    /// names in Rust; anywhere else the keyword was meant as the name.
    #[inline(never)]
    fn keyword_as_name<T>(&mut self) -> ParseResult<T> {
        let token = self.current();
        let spelling = self.source.slice(token.span).unwrap_or_default();
        let rust = token.kind == K::Keyword
            && (self.peek(1) == K::Name || matches!(spelling, "self" | "Self" | "crate" | "super"));
        if !(rust && self.rust_only()) {
            self.diagnostics
                .push(keyword_is_no_name(spelling, token.span));
            // Taken as the name it was meant as, so that recovery does not
            // read a `const` or an `fn` here as the start of a declaration.
            self.bump();
        }
        Err(())
    }

    fn name(&mut self) -> ParseResult<Name> {
        if self.current().kind.is_rust_keyword() {
            return self.keyword_as_name();
        }
        let token = self.expect(K::Name)?;
        Ok(Name {
            text: self.source.slice(token.span).unwrap().to_owned(),
            span: token.span,
        })
    }

    /// `math` and `prop` are ordinary identifiers except where a declaration
    /// or a function type can begin.
    fn at_word(&self, word: &str) -> bool {
        self.at(K::Name) && self.source.slice(self.current().span) == Some(word)
    }

    fn at_math_fn(&self) -> bool {
        self.at_word("math") && self.peek(1) == K::Fn
    }

    fn declaration_start(&self) -> bool {
        matches!(
            self.current().kind,
            K::Fn | K::Def | K::Const | K::Struct | K::Enum
        ) || self.at_math_fn()
            || (self.at_word("prop") && self.peek(1) == K::Name)
    }

    /// Inside delimiters a `{` can no longer be mistaken for a following block.
    fn unrestricted<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<T> {
        let saved = (self.no_struct, self.for_header);
        (self.no_struct, self.for_header) = (false, false);
        let result = operation(self);
        (self.no_struct, self.for_header) = saved;
        result
    }

    fn header<T>(
        &mut self,
        for_header: bool,
        operation: impl FnOnce(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<T> {
        let saved = (self.no_struct, self.for_header);
        (self.no_struct, self.for_header) = (true, for_header);
        let result = operation(self);
        (self.no_struct, self.for_header) = saved;
        result
    }

    fn declaration(&mut self) -> ParseResult<Declaration> {
        // Each attribute is reported and skipped, and its item is parsed.
        while self.attribute_start() {
            self.step();
            if !self.attribute() || self.at(K::Eof) {
                return Err(());
            }
        }
        if !self.declaration_start() {
            return self.fail(
                "expected a declaration: `fn`, `math fn`, `struct`, `enum`, `prop`, or `const`",
            );
        }
        let start = self.bump();
        match start.kind {
            K::Fn => self.function(start, FunctionMode::Runtime),
            K::Def => {
                self.diagnostics.push(
                    Diagnostic::error("L0113", "`def` was renamed to `math fn`", start.span)
                        .note("a `math fn` is pure and total, so it can be used in propositions; it also runs when its body is executable")
                        .suggest(Suggestion {
                            message: "write `math fn`".into(),
                            span: start.span,
                            replacement: "math fn".into(),
                            applicability: Applicability::MachineApplicable,
                        }),
                );
                self.function(start, FunctionMode::Math)
            }
            K::Struct => {
                let name = self.name()?;
                self.no_generics()?;
                let (fields, end) = self.parameter_list(K::LBrace, K::RBrace)?;
                Ok(Declaration {
                    span: start.span.through(end.span),
                    kind: DeclarationKind::Struct { name, fields },
                })
            }
            K::Enum => {
                let name = self.name()?;
                self.no_generics()?;
                let opening = self.expect(K::LBrace)?;
                let mut variants = Vec::new();
                while !self.at(K::RBrace) && !self.at(K::Eof) {
                    self.step();
                    let name = self.name()?;
                    let (fields, end) = self.variant_fields(name.span)?;
                    variants.push(Variant {
                        span: name.span.through(end),
                        name,
                        fields,
                    });
                    if self.eat(K::Comma).is_none() {
                        break;
                    }
                }
                let end = self.close(K::RBrace, opening)?;
                Ok(Declaration {
                    span: start.span.through(end.span),
                    kind: DeclarationKind::Enum { name, variants },
                })
            }
            K::Const => {
                let name = self.name()?;
                self.expect(K::Colon)?;
                let ty = self.ty()?;
                self.expect(K::Equal)?;
                let value = self.expression()?;
                let end = self.semicolon(value.span, "constant declaration")?;
                Ok(Declaration {
                    span: start.span.through(end.span),
                    kind: DeclarationKind::Constant { name, ty, value },
                })
            }
            // `declaration_start` leaves the two contextual words.
            _ if self.at(K::Fn) => {
                self.bump();
                self.function(start, FunctionMode::Math)
            }
            _ => self.prop(start),
        }
    }

    fn function(&mut self, start: Token, mode: FunctionMode) -> ParseResult<Declaration> {
        let name = self.name()?;
        self.no_generics()?;
        let parameters = self.parameters()?;
        self.expect(K::Arrow)?;
        let result = self.ty()?;
        let body = self.block()?;
        Ok(Declaration {
            span: start.span.through(body.span),
            kind: DeclarationKind::Function {
                mode,
                name,
                parameters,
                result,
                body,
            },
        })
    }

    /// `<` after the name an item declares.
    fn no_generics(&mut self) -> ParseResult<()> {
        if self.at(K::Less) {
            self.diagnostics.push(Diagnostic::error(
                "L0116",
                "generic parameters are not in Locus yet",
                self.current().span,
            ));
            return Err(());
        }
        Ok(())
    }

    fn prop(&mut self, start: Token) -> ParseResult<Declaration> {
        let name = self.name()?;
        let parameters = if self.at(K::LParen) {
            self.parameters()?
        } else {
            Vec::new()
        };
        let opening = self.expect(K::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            let name = self.name()?;
            let (fields, mut end) = self.variant_fields(name.span)?;
            let target = if self.eat(K::Colon).is_some() {
                let at = self.expect(K::At)?;
                let target = self.unrestricted(|parser| parser.proof_target(at))?;
                end = target.span;
                Some(target)
            } else {
                None
            };
            variants.push(PropVariant {
                span: name.span.through(end),
                name,
                fields,
                target,
            });
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(K::RBrace, opening)?;
        Ok(Declaration {
            span: start.span.through(end.span),
            kind: DeclarationKind::Prop {
                name,
                parameters,
                variants,
            },
        })
    }

    /// The optional `( fields )` of a variant, and where the variant ends.
    fn variant_fields(&mut self, name: Span) -> ParseResult<(Vec<TypeField>, Span)> {
        if !self.at(K::LParen) {
            return Ok((Vec::new(), name));
        }
        let (fields, end) = self.type_fields()?;
        Ok((fields, end.span))
    }

    fn type_fields(&mut self) -> ParseResult<(Vec<TypeField>, Token)> {
        let opening = self.expect(K::LParen)?;
        let mut fields = Vec::new();
        while !self.at(K::RParen) && !self.at(K::Eof) {
            self.step();
            fields.push(self.type_field()?);
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(K::RParen, opening)?;
        Ok((fields, end))
    }

    fn parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        Ok(self.parameter_list(K::LParen, K::RParen)?.0)
    }

    fn parameter_list(&mut self, open: K, close: K) -> ParseResult<(Vec<Parameter>, Token)> {
        let opening = self.expect(open)?;
        let mut parameters = Vec::new();
        while !self.at(close) && !self.at(K::Eof) {
            self.step();
            let name = self.name()?;
            self.expect(K::Colon)?;
            let ty = self.ty()?;
            parameters.push(Parameter {
                span: name.span.through(ty.span),
                name,
                ty,
            });
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(close, opening)?;
        Ok((parameters, end))
    }

    fn ty(&mut self) -> ParseResult<Type> {
        self.nested(Self::ty_inner)
    }

    fn ty_inner(&mut self) -> ParseResult<Type> {
        let start = self.current();
        match start.kind {
            K::Fn => self.function_type(start, FunctionMode::Runtime),
            K::Name if self.at_math_fn() => {
                self.bump();
                self.function_type(start, FunctionMode::Math)
            }
            K::Name => {
                let name = self.name()?;
                Ok(Type {
                    span: name.span,
                    kind: TypeKind::Named(name),
                })
            }
            K::At => {
                self.bump();
                let proposition = self.proof_target(start)?;
                Ok(Type {
                    span: start.span.through(proposition.span),
                    kind: TypeKind::Proof(Box::new(proposition)),
                })
            }
            K::Hash => self.hash_syntax(),
            K::LBracket => self.no_arrays(start.span),
            K::LParen => {
                self.bump();
                if let Some(end) = self.eat(K::RParen) {
                    return Ok(Type {
                        span: start.span.through(end.span),
                        kind: TypeKind::Unit,
                    });
                }
                let first = self.type_field()?;
                if self.eat(K::Comma).is_none() {
                    if first.name.is_some() && self.at(K::RParen) {
                        let span = first.span.at_end();
                        self.diagnostics.push(
                            Diagnostic::error(
                                "L0106",
                                "a one-field tuple type needs a trailing comma",
                                span,
                            )
                            .suggest(Suggestion {
                                message: "insert `,` after this field".into(),
                                span,
                                replacement: ",".into(),
                                applicability: Applicability::MachineApplicable,
                            }),
                        );
                        return Err(());
                    }
                    let end = self.close(K::RParen, start)?;
                    return Ok(Type {
                        span: start.span.through(end.span),
                        kind: TypeKind::Group(Box::new(first.ty)),
                    });
                }
                let mut fields = vec![first];
                while !self.at(K::RParen) && !self.at(K::Eof) {
                    self.step();
                    fields.push(self.type_field()?);
                    if self.eat(K::Comma).is_none() {
                        break;
                    }
                }
                let end = self.close(K::RParen, start)?;
                Ok(Type {
                    span: start.span.through(end.span),
                    kind: TypeKind::Tuple(fields),
                })
            }
            _ => self.no_type(),
        }
    }

    #[inline(never)]
    fn no_type<T>(&mut self) -> ParseResult<T> {
        if self.current().kind.is_rust_keyword() {
            return self.keyword_as_name();
        }
        self.fail("expected a type such as `u8`, `Prop`, a tuple, or `@[condition]`")
    }

    /// The proposition of a proof type. Calls and projections may follow a
    /// name; other operators need the explicit proposition literal. A proof
    /// type is often followed by a block, so `claim {` is not a struct literal.
    fn proof_target(&mut self, at: Token) -> ParseResult<Expr> {
        if !matches!(self.current().kind, K::Name | K::LBracket | K::LParen) {
            self.diagnostics.push(Diagnostic::error(
                "L0100",
                "a proof type needs a proposition: `@claim` or `@[condition]`",
                at.span,
            ));
            return Err(());
        }
        self.header(false, |parser| parser.expression_bp(11))
    }

    fn function_type(&mut self, start: Token, mode: FunctionMode) -> ParseResult<Type> {
        self.expect(K::Fn)?;
        let (parameters, _) = self.type_fields()?;
        self.expect(K::Arrow)?;
        let result = self.ty()?;
        Ok(Type {
            span: start.span.through(result.span),
            kind: TypeKind::Function {
                mode,
                parameters,
                result: Box::new(result),
            },
        })
    }

    fn no_arrays<T>(&mut self, span: Span) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error("L0114", "arrays are not part of the core language", span)
                .note("brackets hold exactly one proposition, as in `[x <= limit]`"),
        );
        Err(())
    }

    /// A keyword before `:` was meant as a name, and `name` says so.
    fn at_name_or_keyword(&self) -> bool {
        self.at(K::Name) || self.current().kind.is_rust_keyword()
    }

    fn type_field(&mut self) -> ParseResult<TypeField> {
        let start = self.current().span;
        let name = if self.peek(1) == K::Colon && self.at_name_or_keyword() {
            let name = self.name()?;
            self.bump();
            Some(name)
        } else {
            None
        };
        let ty = self.ty()?;
        Ok(TypeField {
            span: start.through(ty.span),
            name,
            ty,
        })
    }

    fn pattern(&mut self) -> ParseResult<Pattern> {
        self.nested(Self::pattern_inner)
    }

    fn pattern_inner(&mut self) -> ParseResult<Pattern> {
        let start = self.current();
        match start.kind {
            K::Name if self.peek(1) == K::PathSep => {
                let path = self.path()?;
                let (arguments, end) = if self.at(K::LParen) {
                    let opening = self.bump();
                    let mut arguments = Vec::new();
                    while !self.at(K::RParen) && !self.at(K::Eof) {
                        self.step();
                        arguments.push(self.pattern()?);
                        if self.eat(K::Comma).is_none() {
                            break;
                        }
                    }
                    let end = self.close(K::RParen, opening)?;
                    (Some(arguments), end.span)
                } else {
                    (None, path.span)
                };
                Ok(Pattern {
                    span: path.span.through(end),
                    kind: PatternKind::Variant {
                        path: Box::new(path),
                        arguments,
                    },
                })
            }
            K::Name if self.peek(1) == K::LBrace => {
                let name = self.name()?;
                let opening = self.bump();
                let mut fields = Vec::new();
                while !self.at(K::RBrace) && !self.at(K::Eof) {
                    self.step();
                    let start = self.current().span;
                    let name = if self.peek(1) == K::Colon {
                        let name = self.name()?;
                        self.bump();
                        Some(name)
                    } else {
                        None
                    };
                    let pattern = self.pattern()?;
                    fields.push(PatternField {
                        span: start.through(pattern.span),
                        name,
                        pattern,
                    });
                    if self.eat(K::Comma).is_none() {
                        break;
                    }
                }
                let end = self.close(K::RBrace, opening)?;
                Ok(Pattern {
                    span: name.span.through(end.span),
                    kind: PatternKind::Struct { name, fields },
                })
            }
            K::Name => {
                let name = self.name()?;
                Ok(Pattern {
                    span: name.span,
                    kind: PatternKind::Name(name),
                })
            }
            K::True | K::False => {
                self.bump();
                Ok(Pattern {
                    span: start.span,
                    kind: PatternKind::Bool(start.kind == K::True),
                })
            }
            K::Integer => Ok(self.integer_pattern()),
            K::Underscore => {
                self.bump();
                Ok(Pattern {
                    span: start.span,
                    kind: PatternKind::Wildcard,
                })
            }
            K::LParen => {
                self.bump();
                if let Some(end) = self.eat(K::RParen) {
                    return Ok(Pattern {
                        span: start.span.through(end.span),
                        kind: PatternKind::Unit,
                    });
                }
                let first = self.pattern()?;
                if self.eat(K::Comma).is_none() {
                    let end = self.close(K::RParen, start)?;
                    return Ok(Pattern {
                        span: start.span.through(end.span),
                        kind: PatternKind::Group(Box::new(first)),
                    });
                }
                let mut patterns = vec![first];
                while !self.at(K::RParen) && !self.at(K::Eof) {
                    self.step();
                    patterns.push(self.pattern()?);
                    if self.eat(K::Comma).is_none() {
                        break;
                    }
                }
                let end = self.close(K::RParen, start)?;
                Ok(Pattern {
                    span: start.span.through(end.span),
                    kind: PatternKind::Tuple(patterns),
                })
            }
            _ => self.no_pattern(),
        }
    }

    #[inline(never)]
    fn no_pattern<T>(&mut self) -> ParseResult<T> {
        if self.current().kind.is_rust_keyword() {
            return self.keyword_as_name();
        }
        self.fail("expected a pattern: a name, `_`, a literal, a tuple, or a constructor")
    }

    #[inline(never)]
    fn integer_pattern(&mut self) -> Pattern {
        let token = self.bump();
        Pattern {
            span: token.span,
            kind: PatternKind::Integer(self.integer(token)),
        }
    }

    fn path(&mut self) -> ParseResult<Path> {
        let prefix = self.name()?;
        self.expect(K::PathSep)?;
        let name = self.name()?;
        Ok(Path {
            span: prefix.span.through(name.span),
            prefix,
            name,
        })
    }

    fn block(&mut self) -> ParseResult<Block> {
        self.nested(|parser| parser.unrestricted(Self::block_inner))
    }

    #[inline(never)]
    fn block_inner(&mut self) -> ParseResult<Block> {
        let opening = self.expect(K::LBrace)?;
        let mut statements = Vec::new();
        let mut tail = None;
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            // A declaration here usually means the preceding function lost its `}`.
            if self.declaration_start() {
                self.close(K::RBrace, opening)?;
            }
            let start = self.position;
            let start_span = self.current().span;
            let result = if self.at(K::Let) {
                self.let_statement()
                    .map(|statement| statements.push(statement))
            } else {
                match self.expression() {
                    Ok(expression) => {
                        if let Some(end) = self.eat(K::Semicolon) {
                            statements.push(Statement {
                                span: expression.span.through(end.span),
                                kind: StatementKind::Expression(expression),
                            });
                            Ok(())
                        } else if self.at(K::RBrace) || self.at(K::Eof) {
                            tail = Some(Box::new(expression));
                            break;
                        } else {
                            self.semicolon(expression.span, "expression").map(|_| ())
                        }
                    }
                    Err(()) => Err(()),
                }
            };
            if result.is_err() {
                self.recover_statement();
                statements.push(Statement {
                    kind: StatementKind::Error,
                    span: start_span.through(self.current().span),
                });
            }
            if self.position == start && !self.at(K::RBrace) && !self.at(K::Eof) {
                self.bump();
            }
        }
        let closing = self.close(K::RBrace, opening)?;
        Ok(Block {
            statements,
            tail,
            span: opening.span.through(closing.span),
        })
    }

    #[inline(never)]
    fn let_statement(&mut self) -> ParseResult<Statement> {
        let opening = self.expect(K::Let)?;
        let pattern = self.pattern()?;
        let annotation = if self.eat(K::Colon).is_some() {
            Some(self.ty()?)
        } else {
            None
        };
        self.expect(K::Equal)?;
        let value = self.expression()?;
        let closing = self.semicolon(value.span, "binding")?;
        Ok(Statement {
            span: opening.span.through(closing.span),
            kind: StatementKind::Let {
                pattern,
                annotation,
                value,
            },
        })
    }

    fn semicolon(&mut self, previous: Span, context: &str) -> ParseResult<Token> {
        if let Some(token) = self.eat(K::Semicolon) {
            return Ok(token);
        }
        let span = previous.at_end();
        self.diagnostics.push(
            Diagnostic::error("L0102", format!("expected `;` after this {context}"), span)
                .label(previous, format!("this {context} ends here"))
                .suggest(Suggestion {
                    message: "insert `;` here".into(),
                    span,
                    replacement: ";".into(),
                    applicability: Applicability::MachineApplicable,
                }),
        );
        Err(())
    }

    fn expression(&mut self) -> ParseResult<Expr> {
        self.expression_bp(0)
    }

    fn expression_bp(&mut self, minimum: u8) -> ParseResult<Expr> {
        self.nested(|parser| parser.expression_bp_inner(minimum))
    }

    // Debug builds give every local its own stack slot, so the recursive
    // functions below stay small and leave node construction to helpers.
    fn expression_bp_inner(&mut self, minimum: u8) -> ParseResult<Expr> {
        let mut left = self.prefix()?;
        let mut chain = 0;
        loop {
            self.step();
            if chain >= MAX_EXPRESSION_CHAIN {
                return self.chain_limit();
            }
            chain += 1;
            if minimum <= 11 && self.at(K::LParen) {
                if self.for_header && self.state_list_follows() {
                    break;
                }
                left = self.call(left)?;
                continue;
            }
            if minimum <= 11 && self.at(K::Dot) {
                left = self.member(left)?;
                continue;
            }
            if self.operator_not_in_locus() {
                return Err(());
            }
            let Some((operator, left_bp, right_bp)) = binary(self.current().kind) else {
                break;
            };
            if left_bp < minimum {
                break;
            }
            left = self.binary(left, operator, right_bp)?;
        }
        Ok(left)
    }

    #[inline(never)]
    fn chain_limit<T>(&mut self) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error(
                "L0108",
                "expression chain exceeds the parser limit",
                self.current().span,
            )
            .note("split this expression using local bindings"),
        );
        Err(())
    }

    /// After an operand: reports an arithmetic operator, or whatever else of
    /// Rust stands here that Locus does not use yet.
    #[inline(never)]
    fn operator_not_in_locus(&mut self) -> bool {
        let operator = self.current();
        if !matches!(
            operator.kind,
            K::Plus | K::Minus | K::Star | K::Slash | K::Percent
        ) {
            return self.rust_only();
        }
        let mut diagnostic = Diagnostic::error(
            "L0112",
            format!(
                "{} is not part of the core language",
                operator.kind.description()
            ),
            operator.span,
        );
        let method = match operator.kind {
            K::Plus => Some("wrapping_add"),
            K::Minus => Some("wrapping_sub"),
            _ => None,
        };
        if let Some(method) = method {
            diagnostic = diagnostic.note(format!(
                "u8 arithmetic says what happens on overflow: write `a.{method}(b)`"
            ));
        }
        self.diagnostics.push(diagnostic);
        true
    }

    #[inline(never)]
    fn call(&mut self, callee: Expr) -> ParseResult<Expr> {
        let (arguments, closing) = self.arguments()?;
        Ok(Expr {
            span: callee.span.through(closing.span),
            kind: ExprKind::Call {
                callee: Box::new(callee),
                arguments,
            },
        })
    }

    #[inline(never)]
    fn member(&mut self, value: Expr) -> ParseResult<Expr> {
        self.expect(K::Dot)?;
        if let Some(index) = self.eat(K::Integer) {
            let spelling = self.source.slice(index.span).unwrap();
            if !spelling.bytes().all(|byte| byte.is_ascii_digit()) {
                self.diagnostics.push(Diagnostic::error(
                    "L0100",
                    "a tuple position is written in decimal digits alone, as in `pair.0`",
                    index.span,
                ));
                return Err(());
            }
            return Ok(Expr {
                span: value.span.through(index.span),
                kind: ExprKind::Index {
                    value: Box::new(value),
                    index: spelling.into(),
                    index_span: index.span,
                },
            });
        }
        let name = self.name()?;
        Ok(Expr {
            span: value.span.through(name.span),
            kind: ExprKind::Member {
                value: Box::new(value),
                name,
            },
        })
    }

    #[inline(never)]
    fn binary(&mut self, left: Expr, operator: BinaryOp, right_bp: u8) -> ParseResult<Expr> {
        if operator.is_comparison()
            && matches!(&left.kind, ExprKind::Binary { operator: previous, .. } if previous.is_comparison())
        {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0103",
                    "comparisons cannot be chained without parentheses",
                    self.current().span,
                )
                .label(left.span, "the first comparison is here")
                .note("write each comparison separately and combine them with `&&`"),
            );
            return Err(());
        }
        let token = self.bump();
        let right = self.expression_bp(right_bp)?;
        Ok(Expr {
            span: left.span.through(right.span),
            kind: ExprKind::Binary {
                operator,
                operator_span: token.span,
                left: Box::new(left),
                right: Box::new(right),
            },
        })
    }

    fn prefix(&mut self) -> ParseResult<Expr> {
        match self.current().kind {
            K::Name if self.peek(1) == K::LBrace && !self.no_struct => self.struct_literal(),
            K::Name | K::Integer | K::String | K::True | K::False | K::Underscore | K::Error => {
                self.atom()
            }
            K::Bang => self.not(),
            K::LParen => self.parenthesized(),
            K::LBracket => self.bracketed(),
            K::LBrace => self.block_expression(),
            K::If => self.if_expression(),
            K::Match => self.match_expression(),
            K::Loop => self.loop_expression(),
            K::For => self.for_expression(),
            K::Break => self.break_expression(),
            K::Continue => self.continue_expression(),
            K::Forall | K::Exists => self.quantifier(),
            K::At => self.proof(),
            K::Hash => self.hash_syntax(),
            _ => self.fail("expected an expression"),
        }
    }

    #[inline(never)]
    fn atom(&mut self) -> ParseResult<Expr> {
        if self.at(K::Name) && self.peek(1) == K::PathSep {
            let path = self.path()?;
            return Ok(Expr {
                span: path.span,
                kind: ExprKind::Path(Box::new(path)),
            });
        }
        if self.at(K::Name) {
            let name = self.name()?;
            return Ok(Expr {
                span: name.span,
                kind: ExprKind::Name(name),
            });
        }
        let token = self.bump();
        Ok(Expr {
            span: token.span,
            kind: match token.kind {
                K::Integer => ExprKind::Integer(self.integer(token)),
                K::String => ExprKind::String(self.string(token)),
                K::True | K::False => ExprKind::Bool(token.kind == K::True),
                K::Underscore => ExprKind::Hole,
                _ => ExprKind::Error,
            },
        })
    }

    /// The value the lexer decoded for an integer token.
    fn integer(&self, token: Token) -> IntegerLiteral {
        match self.literals.get(token.literal as usize) {
            Some(Literal::Integer(literal)) => literal.clone(),
            _ => unreachable!("the lexer gives every integer token its value"),
        }
    }

    fn string(&self, token: Token) -> String {
        match self.literals.get(token.literal as usize) {
            Some(Literal::String(value)) => value.clone(),
            _ => unreachable!("the lexer gives every string token its value"),
        }
    }

    #[inline(never)]
    fn not(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        let value = self.expression_bp(9)?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Not(Box::new(value)),
        })
    }

    #[inline(never)]
    fn block_expression(&mut self) -> ParseResult<Expr> {
        let block = self.block()?;
        Ok(Expr {
            span: block.span,
            kind: ExprKind::Block(block),
        })
    }

    #[inline(never)]
    fn break_expression(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        if matches!(self.current().kind, K::Semicolon | K::RBrace | K::Comma) {
            return self.fail("`break` needs the value the loop produces");
        }
        let value = self.expression()?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Break(Box::new(value)),
        })
    }

    #[inline(never)]
    fn continue_expression(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        if !self.at(K::LParen) {
            return self.fail(
                "`continue` needs the next loop state, as in `continue(next)`; write `continue()` when the loop has no state",
            );
        }
        let (arguments, closing) = self.arguments()?;
        Ok(Expr {
            span: start.span.through(closing.span),
            kind: ExprKind::Continue(arguments),
        })
    }

    #[inline(never)]
    fn quantifier(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        let parameters = self.parameters()?;
        if parameters.is_empty() {
            return self.fail(format!(
                "{} needs at least one parameter",
                start.kind.description()
            ));
        }
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: if start.kind == K::Forall {
                ExprKind::Forall { parameters, body }
            } else {
                ExprKind::Exists { parameters, body }
            },
        })
    }

    fn parenthesized(&mut self) -> ParseResult<Expr> {
        self.unrestricted(Self::parenthesized_inner)
    }

    #[inline(never)]
    fn parenthesized_inner(&mut self) -> ParseResult<Expr> {
        let opening = self.expect(K::LParen)?;
        if let Some(end) = self.eat(K::RParen) {
            return Ok(Expr {
                span: opening.span.through(end.span),
                kind: ExprKind::Unit,
            });
        }
        let first = self.expression()?;
        if self.eat(K::Comma).is_none() {
            let closing = self.close(K::RParen, opening)?;
            return Ok(Expr {
                span: opening.span.through(closing.span),
                kind: ExprKind::Group(Box::new(first)),
            });
        }
        let mut elements = vec![first];
        while !self.at(K::RParen) && !self.at(K::Eof) {
            self.step();
            elements.push(self.expression()?);
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let closing = self.close(K::RParen, opening)?;
        Ok(Expr {
            span: opening.span.through(closing.span),
            kind: ExprKind::Tuple(elements),
        })
    }

    #[inline(never)]
    fn if_expression(&mut self) -> ParseResult<Expr> {
        self.nested(|parser| {
            let opening = parser.expect(K::If)?;
            let condition = parser.header(false, Self::expression)?;
            let then_branch = parser.block()?;
            if !parser.at(K::Else) {
                return parser.fail("an `if` expression requires an `else` branch");
            }
            parser.bump();
            let else_branch = if parser.at(K::If) {
                parser.if_expression()?
            } else {
                let block = parser.block()?;
                Expr {
                    span: block.span,
                    kind: ExprKind::Block(block),
                }
            };
            Ok(Expr {
                span: opening.span.through(else_branch.span),
                kind: ExprKind::If {
                    condition: Box::new(condition),
                    then_branch,
                    else_branch: Box::new(else_branch),
                },
            })
        })
    }

    /// `@` begins a proof type; there is no proof expression spelled with it.
    fn proof(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::At)?;
        let mut diagnostic = Diagnostic::error(
            "L0110",
            "`@` begins a proof type, not an expression",
            start.span,
        );
        diagnostic = if self.at(K::LBrace) {
            diagnostic.note("proof blocks `@{ ... }` were retired; evidence is an ordinary expression, and `_` asks the elaborator to find it")
        } else {
            diagnostic.note("`@claim` and `@[condition]` are types; request evidence with `let evidence: @claim = _;`")
        };
        self.diagnostics.push(diagnostic);
        Err(())
    }

    #[inline(never)]
    fn bracketed(&mut self) -> ParseResult<Expr> {
        let opening = self.expect(K::LBracket)?;
        if self.at(K::RBracket) {
            return self.no_arrays(opening.span.through(self.current().span));
        }
        let formula = self.unrestricted(Self::expression)?;
        if matches!(self.current().kind, K::Comma | K::Semicolon) {
            return self.no_arrays(self.current().span);
        }
        let end = self.close(K::RBracket, opening)?;
        Ok(Expr {
            span: opening.span.through(end.span),
            kind: ExprKind::Proposition(Box::new(formula)),
        })
    }

    #[inline(never)]
    fn arguments(&mut self) -> ParseResult<(Vec<Expr>, Token)> {
        self.unrestricted(|parser| {
            let opening = parser.expect(K::LParen)?;
            let mut arguments = Vec::new();
            while !parser.at(K::RParen) && !parser.at(K::Eof) {
                parser.step();
                arguments.push(parser.expression()?);
                if parser.eat(K::Comma).is_none() {
                    break;
                }
            }
            let closing = parser.close(K::RParen, opening)?;
            Ok((arguments, closing))
        })
    }

    /// In the bounds of a `for`, the parenthesized group directly before the
    /// body is the state list, not a call on the upper bound.
    fn state_list_follows(&mut self) -> bool {
        let closers = self
            .closers
            .get_or_insert_with(|| matching_delimiters(&self.tokens));
        closers[self.position].is_some_and(|closer| self.tokens[closer + 1].kind == K::LBrace)
    }

    #[inline(never)]
    fn struct_literal(&mut self) -> ParseResult<Expr> {
        let name = self.name()?;
        let opening = self.expect(K::LBrace)?;
        let fields = self.unrestricted(|parser| {
            let mut fields = Vec::new();
            while !parser.at(K::RBrace) && !parser.at(K::Eof) {
                parser.step();
                let start = parser.current().span;
                let name = if parser.peek(1) == K::Colon {
                    let name = parser.name()?;
                    parser.bump();
                    Some(name)
                } else {
                    None
                };
                let value = parser.expression()?;
                fields.push(ValueField {
                    span: start.through(value.span),
                    name,
                    value,
                });
                if parser.eat(K::Comma).is_none() {
                    break;
                }
            }
            Ok(fields)
        })?;
        let end = self.close(K::RBrace, opening)?;
        Ok(Expr {
            span: name.span.through(end.span),
            kind: ExprKind::Struct { name, fields },
        })
    }

    #[inline(never)]
    fn match_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::Match)?;
        let scrutinee = self.header(false, Self::expression)?;
        let opening = self.expect(K::LBrace)?;
        let arms = self.unrestricted(|parser| {
            let mut arms = Vec::new();
            while !parser.at(K::RBrace) && !parser.at(K::Eof) {
                parser.step();
                let pattern = parser.pattern()?;
                if !parser.at(K::Implies) {
                    return parser.fail("expected `=>` between a match arm's pattern and its body");
                }
                parser.bump();
                let body = parser.expression()?;
                let block_like = matches!(body.kind, ExprKind::Block(_));
                arms.push(MatchArm {
                    span: pattern.span.through(body.span),
                    pattern,
                    body,
                });
                // As in Rust, the comma is optional after an arm that is a block.
                if parser.eat(K::Comma).is_none() && !block_like {
                    break;
                }
            }
            Ok(arms)
        })?;
        let end = self.close(K::RBrace, opening)?;
        Ok(Expr {
            span: start.span.through(end.span),
            kind: ExprKind::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
        })
    }

    #[inline(never)]
    fn loop_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::Loop)?;
        if !self.at(K::LParen) {
            return self.fail(
                "a `loop` lists its state and result type: `loop (state: T = initial) -> R { ... }`",
            );
        }
        let state = self.state_parameters()?;
        self.expect(K::Arrow)?;
        let result = self.ty()?;
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: ExprKind::Loop {
                state,
                result: Box::new(result),
                body,
            },
        })
    }

    #[inline(never)]
    fn for_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::For)?;
        let index = self.name()?;
        self.expect(K::In)?;
        let lower = self.header(true, Self::expression)?;
        self.expect(K::DotDot)?;
        let upper = self.header(true, Self::expression)?;
        if !self.at(K::LParen) {
            return self.fail(
                "a `for` lists its state before the body: `for i in lo..hi (state: T = initial) { ... }`; write `()` when there is none",
            );
        }
        let state = self.state_parameters()?;
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: ExprKind::For {
                index,
                lower: Box::new(lower),
                upper: Box::new(upper),
                state,
                body,
            },
        })
    }

    #[inline(never)]
    fn state_parameters(&mut self) -> ParseResult<Vec<StateParameter>> {
        self.unrestricted(|parser| {
            let opening = parser.expect(K::LParen)?;
            let mut state = Vec::new();
            while !parser.at(K::RParen) && !parser.at(K::Eof) {
                parser.step();
                let name = parser.name()?;
                parser.expect(K::Colon)?;
                let ty = parser.ty()?;
                parser.expect(K::Equal)?;
                let initial = parser.expression()?;
                state.push(StateParameter {
                    span: name.span.through(initial.span),
                    name,
                    ty,
                    initial,
                });
                if parser.eat(K::Comma).is_none() {
                    break;
                }
            }
            parser.close(K::RParen, opening)?;
            Ok(state)
        })
    }

    fn hash_syntax<T>(&mut self) -> ParseResult<T> {
        if self.attribute_start() {
            return self.unsupported_attribute();
        }
        self.diagnostics.push(Diagnostic::error(
            "L0111", "`#` is no longer proof syntax", self.current().span,
        ).note("use `@claim` or `@[condition]` for proof types, and `_` to ask the elaborator for evidence"));
        Err(())
    }

    fn attribute_start(&self) -> bool {
        self.at(K::Hash)
            && (self.peek(1) == K::LBracket
                || (self.peek(1) == K::Bang && self.peek(2) == K::LBracket))
    }

    fn unsupported_attribute<T>(&mut self) -> ParseResult<T> {
        self.attribute();
        Err(())
    }

    /// Reports one attribute and skips it, brackets and all, so that it is
    /// reported once. False when its bracket is never closed.
    fn attribute(&mut self) -> bool {
        let start = self.bump();
        self.eat(K::Bang);
        let closers = self
            .closers
            .get_or_insert_with(|| matching_delimiters(&self.tokens));
        let closer = closers[self.position];
        let end = closer.map_or(self.current(), |closer| self.tokens[closer]);
        self.diagnostics.push(
            Diagnostic::error(
                "L0105",
                "attributes are not in Locus yet",
                start.span.through(end.span),
            )
            .note("proof types use `@claim` or `@[condition]`, and `_` asks the elaborator for evidence"),
        );
        if let Some(closer) = closer {
            self.position = closer + 1;
        }
        closer.is_some()
    }

    /// An attribute, or a keyword that begins an item in Rust and not yet in
    /// Locus.
    fn rust_item_start(&self) -> bool {
        if self.attribute_start() {
            return true;
        }
        self.at(K::Keyword)
            && matches!(
                self.source.slice(self.current().span),
                Some(
                    "async"
                        | "extern"
                        | "impl"
                        | "mod"
                        | "pub"
                        | "static"
                        | "trait"
                        | "type"
                        | "unsafe"
                        | "use"
                )
            )
    }

    /// Skips to where the next declaration begins. An item of Rust is a place
    /// to stop as well, past the one that failed, so that each is reported.
    fn recover_declaration(&mut self) {
        let start = self.position;
        let mut depth = 0usize;
        while !self.at(K::Eof) {
            self.step();
            let kind = self.current().kind;
            if depth == 0
                && (self.declaration_start() || (self.position > start && self.rust_item_start()))
            {
                return;
            }
            match kind {
                K::LParen | K::LBrace | K::LBracket => depth += 1,
                K::RParen | K::RBrace | K::RBracket => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }
    }

    fn recover_statement(&mut self) {
        let mut depth = 0usize;
        while !self.at(K::Eof) {
            self.step();
            let kind = self.current().kind;
            if depth == 0 {
                if kind == K::RBrace || kind == K::Let || self.declaration_start() {
                    return;
                }
                if kind == K::Semicolon {
                    self.bump();
                    return;
                }
            }
            match kind {
                K::LParen | K::LBrace | K::LBracket => depth += 1,
                K::RParen | K::RBrace | K::RBracket => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }
    }
}

/// Pairs delimiters by depth alone, as the recovery loops do: `(`, `{`, and
/// `[` open, and any closer closes the nearest open one. One pass over the
/// tokens.
fn matching_delimiters(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut closers = vec![None; tokens.len()];
    let mut open = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            K::LParen | K::LBrace | K::LBracket => open.push(index),
            K::RParen | K::RBrace | K::RBracket => {
                if let Some(opener) = open.pop() {
                    closers[opener] = Some(index);
                }
            }
            _ => {}
        }
    }
    closers
}

/// L0115. Locus reserves every strict and reserved keyword of Rust, the ones
/// it has no use for included, so that no Locus source reads differently as
/// Rust.
fn keyword_is_no_name(keyword: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "L0115",
        format!("`{keyword}` is a Rust keyword and cannot be used as a name"),
        span,
    )
    .note("Locus reserves every keyword of Rust, whether or not it uses it yet")
    .suggest(Suggestion {
        message: format!("rename it, for example to `{keyword}_`"),
        span,
        replacement: format!("{keyword}_"),
        applicability: Applicability::MaybeIncorrect,
    })
}

/// What a keyword that Locus does not use yet begins or marks in Rust, and a
/// note where Locus has another way. `None` for the keywords Rust reserves
/// without a use of its own.
fn keyword_construct(keyword: &str) -> Option<(&'static str, Option<&'static str>)> {
    let message = match keyword {
        "as" => "`as` casts are not in Locus yet",
        "async" => "`async` is not in Locus yet",
        "await" => "`await` is not in Locus yet",
        "crate" => "`crate` paths are not in Locus yet",
        "dyn" => "`dyn` trait objects are not in Locus yet",
        "extern" => "`extern` is not in Locus yet",
        "impl" => "`impl` blocks and `impl Trait` are not in Locus yet",
        "mod" => "modules (`mod`) are not in Locus yet",
        "move" => "closures (`move`) are not in Locus yet",
        "mut" => "`mut` is not in Locus yet",
        "pub" => "visibility (`pub`) is not in Locus yet",
        "ref" => "`ref` bindings are not in Locus yet",
        "return" => "`return` is not in Locus yet",
        "self" => "`self` and methods are not in Locus yet",
        "Self" => "`Self` is not in Locus yet",
        "static" => "`static` items are not in Locus yet",
        "super" => "`super` paths are not in Locus yet",
        "trait" => "traits are not in Locus yet",
        "type" => "type aliases (`type`) are not in Locus yet",
        "unsafe" => "`unsafe` is not in Locus yet",
        "use" => "`use` declarations are not in Locus yet",
        "where" => "`where` clauses are not in Locus yet",
        "while" => "`while` loops are not in Locus yet",
        _ => return None,
    };
    let note = match keyword {
        "mut" => Some("a binding never changes; `loop` and `for` carry their state explicitly"),
        "return" => Some("the value of a function is the tail expression of its body"),
        "while" => Some("write a `loop`, or a `for` over a range, with its state listed"),
        _ => None,
    };
    Some((message, note))
}

/// L0116 for a token that is an operator or punctuation of Rust alone.
fn rust_only_token(kind: K, spelling: &str) -> Option<String> {
    Some(match kind {
        K::PlusEqual
        | K::MinusEqual
        | K::StarEqual
        | K::SlashEqual
        | K::PercentEqual
        | K::CaretEqual
        | K::AndEqual
        | K::OrEqual
        | K::ShiftLeftEqual
        | K::ShiftRightEqual => {
            format!("compound assignment (`{spelling}`) is not in Locus yet")
        }
        K::ShiftLeft | K::ShiftRight => {
            format!("the shift operator `{spelling}` is not in Locus yet")
        }
        K::And => "references and the `&` operator are not in Locus yet".into(),
        K::Or => "closures, or-patterns, and the `|` operator are not in Locus yet".into(),
        K::Caret => "the `^` operator is not in Locus yet".into(),
        K::Minus => "negation (`-`) is not in Locus yet".into(),
        K::Star => "dereferences and raw pointers (`*`) are not in Locus yet".into(),
        K::Question => "the `?` operator is not in Locus yet".into(),
        K::Dollar => "`$` belongs to macros, which are not in Locus yet".into(),
        K::DotDotEqual => "inclusive ranges (`..=`) are not in Locus yet".into(),
        K::DotDotDot => "`...` is not in Locus yet".into(),
        K::Tilde | K::LeftArrow => {
            format!("`{spelling}` is a token of Rust with no meaning in Locus")
        }
        _ => return None,
    })
}

fn binary(kind: K) -> Option<(BinaryOp, u8, u8)> {
    Some(match kind {
        K::Implies => (BinaryOp::Implies, 1, 1),
        K::OrOr => (BinaryOp::Or, 2, 3),
        K::AndAnd => (BinaryOp::And, 3, 4),
        K::EqualEqual => (BinaryOp::Equal, 5, 6),
        K::BangEqual => (BinaryOp::NotEqual, 5, 6),
        K::Less => (BinaryOp::Less, 5, 6),
        K::LessEqual => (BinaryOp::LessEqual, 5, 6),
        K::Greater => (BinaryOp::Greater, 5, 6),
        K::GreaterEqual => (BinaryOp::GreaterEqual, 5, 6),
        _ => return None,
    })
}
