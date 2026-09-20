//! Recursive-descent syntax parser with Pratt expression precedence.

use crate::ast::*;
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::lexer::{Token, TokenKind as K, lex};
use crate::source::{SourceFile, Span};

const MAX_DEPTH: usize = 128;
const MAX_EXPRESSION_CHAIN: usize = 128;
type ParseResult<T> = Result<T, ()>;

#[derive(Debug)]
pub struct Parsed {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
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
        position: 0,
        depth: 0,
        diagnostics: lexed.diagnostics,
    }
    .program()
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    position: usize,
    depth: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn program(mut self) -> Parsed {
        let mut program = Program::default();
        while !self.at(K::Eof) {
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
        }
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

    fn fail<T>(&mut self, message: impl Into<String>) -> ParseResult<T> {
        if !self.at(K::Error) {
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
        self.depth += 1;
        let result = operation(self);
        self.depth -= 1;
        result
    }

    fn name(&mut self) -> ParseResult<Name> {
        let token = self.expect(K::Name)?;
        Ok(Name {
            text: self.source.slice(token.span).unwrap().to_owned(),
            span: token.span,
        })
    }

    fn declaration(&mut self) -> ParseResult<Declaration> {
        if self.attribute_start() {
            return self.unsupported_attribute();
        }
        let start = self.current();
        if !matches!(start.kind, K::Fn | K::Def | K::Const) {
            return self.fail("expected a function (`fn`), logical definition (`def`), or constant (`const`) declaration");
        }
        self.bump();
        let name = self.name()?;
        if matches!(start.kind, K::Fn | K::Def) {
            let parameters = self.parameters()?;
            self.expect(K::Arrow)?;
            let result = self.ty()?;
            let body = self.block()?;
            Ok(Declaration {
                span: start.span.through(body.span),
                kind: DeclarationKind::Function {
                    mode: if start.kind == K::Def {
                        FunctionMode::Logical
                    } else {
                        FunctionMode::Runtime
                    },
                    name,
                    parameters,
                    result,
                    body,
                },
            })
        } else {
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
    }

    fn parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        let opening = self.expect(K::LParen)?;
        let mut parameters = Vec::new();
        while !self.at(K::RParen) && !self.at(K::Eof) {
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
        self.close(K::RParen, opening)?;
        Ok(parameters)
    }

    fn ty(&mut self) -> ParseResult<Type> {
        self.nested(Self::ty_inner)
    }

    fn ty_inner(&mut self) -> ParseResult<Type> {
        let start = self.current();
        match start.kind {
            K::Name => {
                let name = self.name()?;
                Ok(Type {
                    span: name.span,
                    kind: TypeKind::Named(name),
                })
            }
            K::At => {
                self.bump();
                if !matches!(self.current().kind, K::Name | K::LBracket | K::LParen) {
                    return self
                        .fail("a proof type needs a proposition: `@claim` or `@[condition]`");
                }
                // Calls/member access may form a proposition; ungrouped binary
                // operators cannot escape the explicit proposition literal.
                let proposition = self.expression_bp(11)?;
                Ok(Type {
                    span: start.span.through(proposition.span),
                    kind: TypeKind::Proof(Box::new(proposition)),
                })
            }
            K::Hash => self.hash_syntax(),
            K::LBracket => {
                self.bump();
                let element = Box::new(self.ty()?);
                let kind = if self.eat(K::Semicolon).is_some() {
                    TypeKind::Array {
                        element,
                        length: Box::new(self.expression()?),
                    }
                } else {
                    TypeKind::Slice(element)
                };
                let end = self.close(K::RBracket, start)?;
                Ok(Type {
                    span: start.span.through(end.span),
                    kind,
                })
            }
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
            _ => self.fail("expected a type such as `Nat`, `Prop`, a tuple, or `@[condition]`"),
        }
    }

    fn type_field(&mut self) -> ParseResult<TypeField> {
        let start = self.current().span;
        let name = if self.at(K::Name) && self.peek(1) == K::Colon {
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
            K::Name => {
                let name = self.name()?;
                Ok(Pattern {
                    span: name.span,
                    kind: PatternKind::Name(name),
                })
            }
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
            _ => self.fail("expected a binding name, `_`, or tuple pattern"),
        }
    }

    fn block(&mut self) -> ParseResult<Block> {
        self.nested(Self::block_inner)
    }

    fn block_inner(&mut self) -> ParseResult<Block> {
        let opening = self.expect(K::LBrace)?;
        let mut statements = Vec::new();
        let mut tail = None;
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            // A declaration here usually means the preceding function lost its `}`.
            if matches!(self.current().kind, K::Fn | K::Def | K::Const) {
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

    fn expression_bp_inner(&mut self, minimum: u8) -> ParseResult<Expr> {
        let mut left = self.prefix()?;
        let mut chain = 0;
        loop {
            if chain >= MAX_EXPRESSION_CHAIN {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0108",
                        "expression chain exceeds the parser limit",
                        self.current().span,
                    )
                    .note("split this expression using local bindings"),
                );
                return Err(());
            }
            if minimum <= 11 && self.at(K::LParen) {
                let opening = self.bump();
                let mut arguments = Vec::new();
                while !self.at(K::RParen) && !self.at(K::Eof) {
                    arguments.push(self.expression()?);
                    if self.eat(K::Comma).is_none() {
                        break;
                    }
                }
                let closing = self.close(K::RParen, opening)?;
                left = Expr {
                    span: left.span.through(closing.span),
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                };
                chain += 1;
                continue;
            }
            if minimum <= 11 && self.eat(K::Dot).is_some() {
                let name = self.name()?;
                left = Expr {
                    span: left.span.through(name.span),
                    kind: ExprKind::Member {
                        value: Box::new(left),
                        name,
                    },
                };
                chain += 1;
                continue;
            }
            let Some((operator, left_bp, right_bp)) = binary(self.current().kind) else {
                break;
            };
            if left_bp < minimum {
                break;
            }
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
            left = Expr {
                span: left.span.through(right.span),
                kind: ExprKind::Binary {
                    operator,
                    operator_span: token.span,
                    left: Box::new(left),
                    right: Box::new(right),
                },
            };
            chain += 1;
        }
        Ok(left)
    }

    fn prefix(&mut self) -> ParseResult<Expr> {
        let start = self.current();
        match start.kind {
            K::Name => {
                let name = self.name()?;
                Ok(Expr {
                    span: name.span,
                    kind: ExprKind::Name(name),
                })
            }
            K::Integer => {
                self.bump();
                Ok(Expr {
                    span: start.span,
                    kind: ExprKind::Integer(self.source.slice(start.span).unwrap().into()),
                })
            }
            K::True | K::False => {
                self.bump();
                Ok(Expr {
                    span: start.span,
                    kind: ExprKind::Bool(start.kind == K::True),
                })
            }
            K::Bang => {
                self.bump();
                let value = self.expression_bp(9)?;
                Ok(Expr {
                    span: start.span.through(value.span),
                    kind: ExprKind::Not(Box::new(value)),
                })
            }
            K::LParen => self.parenthesized(),
            K::LBracket => self.bracketed(),
            K::LBrace => {
                let block = self.block()?;
                Ok(Expr {
                    span: block.span,
                    kind: ExprKind::Block(block),
                })
            }
            K::If => self.if_expression(),
            K::Forall => {
                self.bump();
                let parameters = self.parameters()?;
                if parameters.is_empty() {
                    return self.fail("`forall` needs at least one parameter");
                }
                let body = self.block()?;
                Ok(Expr {
                    span: start.span.through(body.span),
                    kind: ExprKind::Forall { parameters, body },
                })
            }
            K::At => self.proof(),
            K::Hash => self.hash_syntax(),
            K::Underscore => {
                self.bump();
                Ok(Expr {
                    span: start.span,
                    kind: ExprKind::Proof(ProofRequest::Inferred),
                })
            }
            K::Error => {
                self.bump();
                Ok(Expr {
                    span: start.span,
                    kind: ExprKind::Error,
                })
            }
            _ => self.fail("expected an expression"),
        }
    }

    fn parenthesized(&mut self) -> ParseResult<Expr> {
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

    fn if_expression(&mut self) -> ParseResult<Expr> {
        self.nested(|parser| {
            let opening = parser.expect(K::If)?;
            let condition = parser.expression()?;
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

    fn proof(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::At)?;
        if !self.at(K::LBrace) {
            self.diagnostics.push(Diagnostic::error(
                "L0110", "a proof expression uses `@{ ... }` or `_`", start.span,
            ).note("`@claim` and `@[condition]` are types; request evidence with `let evidence: @claim = _;`"));
            return Err(());
        }
        let opening = self.bump();
        let mut commands = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            // Let the enclosing block recover at the next declaration/binding.
            if matches!(self.current().kind, K::Fn | K::Def | K::Const | K::Let) {
                self.close(K::RBrace, opening)?;
            }
            let before = self.position;
            match self.proof_command() {
                Ok(command) => commands.push(command),
                Err(()) => self.recover_statement(),
            }
            if self.position == before && !self.at(K::RBrace) && !self.at(K::Eof) {
                self.bump();
            }
        }
        let end = self.close(K::RBrace, opening)?;
        Ok(Expr {
            span: start.span.through(end.span),
            kind: ExprKind::Proof(ProofRequest::Block { commands }),
        })
    }

    fn bracketed(&mut self) -> ParseResult<Expr> {
        let opening = self.expect(K::LBracket)?;
        if let Some(end) = self.eat(K::RBracket) {
            return Ok(Expr {
                span: opening.span.through(end.span),
                kind: ExprKind::Array(Vec::new()),
            });
        }
        let first = self.expression()?;
        let kind = if self.eat(K::Semicolon).is_some() {
            ExprKind::RepeatArray {
                value: Box::new(first),
                count: Box::new(self.expression()?),
            }
        } else if self.eat(K::Comma).is_some() {
            let mut elements = vec![first];
            while !self.at(K::RBracket) && !self.at(K::Eof) {
                elements.push(self.expression()?);
                if self.eat(K::Comma).is_none() {
                    break;
                }
            }
            ExprKind::Array(elements)
        } else {
            // Keep the contents neutral until an expected type is available.
            ExprKind::Bracket(Box::new(first))
        };
        let end = self.close(K::RBracket, opening)?;
        Ok(Expr {
            span: opening.span.through(end.span),
            kind,
        })
    }

    fn proof_command(&mut self) -> ParseResult<ProofCommand> {
        let name = self.name()?;
        let mut arguments = Vec::new();
        while !matches!(self.current().kind, K::Semicolon | K::RBrace | K::Eof) {
            arguments.push(self.expression()?);
        }
        let previous = arguments.last().map_or(name.span, |argument| argument.span);
        let end = self.semicolon(previous, "proof command")?;
        Ok(ProofCommand {
            span: name.span.through(end.span),
            name,
            arguments,
        })
    }

    fn hash_syntax<T>(&mut self) -> ParseResult<T> {
        if self.attribute_start() {
            return self.unsupported_attribute();
        }
        self.diagnostics.push(Diagnostic::error(
            "L0111", "`#` is no longer proof syntax", self.current().span,
        ).note("use `@claim` or `@[condition]` for proof types, `@{ ... }` for proof blocks, and `_` for automatic proof requests"));
        Err(())
    }

    fn attribute_start(&self) -> bool {
        self.at(K::Hash)
            && (self.peek(1) == K::LBracket
                || (self.peek(1) == K::Bang && self.peek(2) == K::LBracket))
    }

    fn unsupported_attribute<T>(&mut self) -> ParseResult<T> {
        self.diagnostics.push(Diagnostic::error("L0105", "attributes are reserved but not supported in the initial core", self.current().span)
            .note("proof types use `@claim` or `@[condition]`, proof blocks use `@{ ... }`, and automatic proof requests use `_`"));
        Err(())
    }

    fn recover_declaration(&mut self) {
        let mut depth = 0usize;
        while !self.at(K::Eof) {
            let kind = self.current().kind;
            if depth == 0 && matches!(kind, K::Fn | K::Def | K::Const) {
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
            let kind = self.current().kind;
            if depth == 0 {
                if matches!(kind, K::RBrace | K::Let | K::Fn | K::Def | K::Const) {
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
        K::Plus => (BinaryOp::Add, 7, 8),
        _ => return None,
    })
}
