//! Recursive-descent syntax parser with Pratt expression precedence.

use crate::ast::*;
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::lexer::{Literal, Token, TokenKind as K, lex};
use crate::source::{SourceFile, Span};

use crate::limits::MAX_EXPRESSION_CHAIN;
use crate::limits::MAX_PARSER_DEPTH as MAX_DEPTH;
type ParseResult<T> = Result<T, ()>;

/// Rust's precedence table (the Reference, "Expression precedence"), as
/// binding powers from loosest to tightest. A binary operator at `(left,
/// right)` takes the expression on its left when `left` is at least the
/// minimum in force, and reads its right operand with `right` as the
/// minimum: `right` is `left + 1` for the left-associative operators and
/// `left` for `=>`, which is right-associative. The comparisons are not
/// associative: a comparison whose operand is a comparison is an error.
const EVIDENCE: u8 = 0;
const IMPLIES: (u8, u8) = (1, 1);
const OR: (u8, u8) = (2, 3);
const AND: (u8, u8) = (4, 5);
const COMPARISON: (u8, u8) = (6, 7);
const BIT_OR: (u8, u8) = (8, 9);
const BIT_XOR: (u8, u8) = (10, 11);
const BIT_AND: (u8, u8) = (12, 13);
const SHIFT: (u8, u8) = (14, 15);
const SUM: (u8, u8) = (16, 17);
const PRODUCT: (u8, u8) = (18, 19);
/// `as`, which takes a type and not an operand on its right.
const CAST: u8 = 20;
/// The minimum at which only the postfix operators (calls, `.name`, `.0`)
/// apply: the operand of a prefix operator, and the proposition of `@claim`.
const OPERAND: u8 = 21;

/// What `Parser::operator` found after an operand.
#[derive(Clone, Copy)]
enum Operator {
    Subscript,
    Call,
    Member,
    Cast,
    Evidence,
    /// A binary operator, with the binding power of its right operand.
    Binary(BinaryOp, u8),
}

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
    /// No error was reported; warnings do not count.
    pub fn is_success(&self) -> bool {
        !self.diagnostics.iter().any(Diagnostic::is_error)
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
        statement: false,
        formula: false,
        in_impl: false,
        in_method: false,
        closers: None,
        diagnostics: lexed.diagnostics.into(),
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
    /// Set in the header of an `if`, `match`, `while`, or `for`, where
    /// `Name {` begins the following block rather than a struct literal.
    no_struct: bool,
    /// Set in the bounds of a `for`, where `..` ends a bound.
    for_header: bool,
    /// Set by a block for the expression that begins a statement, and taken
    /// by that expression alone: there, `=` ends the place of an assignment,
    /// and an expression that ends in a block is the whole statement.
    statement: bool,
    /// Set inside `prop!(...)`, `prove!(...)`, and `@(...)`, where `=>` is
    /// implication and `forall (` and `exists (` begin quantifiers. Outside
    /// a formula `forall` and `exists` are names, and `=>` is an error.
    formula: bool,
    /// Set inside an `impl` block, where `Self` is a type.
    in_impl: bool,
    /// Set in the body of a method with a `self` parameter, where `self`
    /// is a value.
    in_method: bool,
    /// For each opening delimiter, the token that closes it; built by the
    /// first `for` header or attribute that asks, so that the lookahead is
    /// not a rescan.
    closers: Option<Vec<Option<usize>>>,
    diagnostics: crate::limits::DiagnosticBuffer,
}

impl Parser<'_> {
    fn program(mut self) -> Parsed {
        let mut program = Program::default();
        self.file_header(&mut program);
        while !self.at(K::Eof) && !self.diagnostics.overflowed() {
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
            diagnostics: self.diagnostics.into_vec(),
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
                .note(format!("MAX_PARSER_DEPTH limit of {MAX_DEPTH}; split deeply nested expressions or types into smaller definitions")),
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
            self.misplaced_attribute();
            return true;
        }
        let spelling = self.source.slice(token.span).unwrap_or_default();
        let diagnostic = if token.kind == K::Keyword {
            match keyword_construct(spelling) {
                Some(message) => Diagnostic::error("L0116", message, token.span),
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

    /// A keyword where a name belongs. Before a name, `ref` or `move` is a
    /// construct of Rust that Locus does not have yet; anywhere else the
    /// keyword was meant as the name.
    #[inline(never)]
    fn keyword_as_name<T>(&mut self) -> ParseResult<T> {
        let token = self.current();
        let spelling = self.source.slice(token.span).unwrap_or_default();
        let rust = token.kind == K::Keyword && self.peek(1) == K::Name;
        if !(rust && self.rust_only()) {
            self.diagnostics
                .push(keyword_is_no_name(spelling, token.span));
        }
        // Taken as the name it was meant as, so that recovery does not read
        // a `const`, an `fn`, or an `impl` here as the start of a declaration.
        self.bump();
        Err(())
    }

    fn name(&mut self) -> ParseResult<Name> {
        if self.current().kind.is_keyword() {
            return self.keyword_as_name();
        }
        let token = self.expect(K::Name)?;
        Ok(Name {
            text: self.source.slice(token.span).unwrap().to_owned(),
            span: token.span,
        })
    }

    /// `forall` and `exists` are contextual names before `(` in a formula.
    fn at_word(&self, word: &str) -> bool {
        self.at(K::Name) && self.source.slice(self.current().span) == Some(word)
    }

    /// A keyword of Rust that Locus reads in context: `pub`, `impl`,
    /// `self`, `Self`, `crate`, `super`.
    fn at_keyword(&self, word: &str) -> bool {
        self.at(K::Keyword) && self.source.slice(self.current().span) == Some(word)
    }

    fn spelling(&self) -> &str {
        self.source.slice(self.current().span).unwrap_or_default()
    }

    fn declaration_start(&self) -> bool {
        matches!(self.current().kind, K::Fn | K::Const | K::Struct | K::Enum)
            || (self.at(K::Logic) && self.peek(1) == K::Fn)
            || (self.at(K::Prop) && self.peek(1) != K::Bang)
            || self.at_keyword("pub")
            || self.at_keyword("impl")
            || self.at_word("trusted")
    }

    /// A doc comment or an attribute, which begin an item as well.
    fn item_prefix_start(&self) -> bool {
        self.at(K::OuterDoc) || self.at(K::InnerDoc) || self.attribute_start()
    }

    /// Whether the current token begins a name or a path in a type, an
    /// expression, or a pattern: an identifier, `Self`, or `crate`, `super`,
    /// or `self` before `::`.
    fn at_named(&self) -> bool {
        match self.current().kind {
            K::Name => true,
            K::Keyword => {
                let spelling = self.spelling();
                spelling == "Self"
                    || (matches!(spelling, "crate" | "super" | "self")
                        && self.peek(1) == K::PathSep)
            }
            _ => false,
        }
    }

    /// `name!` before a delimiter: a form, or something reported as not one.
    fn at_form(&self) -> bool {
        self.peek(1) == K::Bang && matches!(self.peek(2), K::LParen | K::LBracket | K::LBrace)
    }

    /// `forall (` or `exists (`, a quantifier inside a formula. Outside one,
    /// `forall (x: T)` can only have been meant as a quantifier, and is
    /// reported as such, while `forall(x)` is a call.
    fn at_quantifier(&self) -> bool {
        (self.formula || self.peek(3) == K::Colon)
            && self.peek(1) == K::LParen
            && (self.at_word("forall") || self.at_word("exists"))
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

    /// A formula: the argument of `prop!` or `prove!`, or the parenthesized
    /// proposition of a proof type.
    fn formula_mode<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<T> {
        let saved = std::mem::replace(&mut self.formula, true);
        let result = operation(self);
        self.formula = saved;
        result
    }

    /// An item: its doc comments, attributes, and visibility, then one of
    /// the declaration forms. An `impl` holds functions and constants.
    fn declaration(&mut self) -> ParseResult<Declaration> {
        let start = self.current().span;
        let (doc, mut attributes) = self.outer_attributes()?;
        let visibility = self.visibility()?;
        if self.at_word("trusted") {
            let (kind, attribute, end) = self.trusted_function()?;
            attributes.push(attribute);
            return Ok(Declaration {
                doc,
                attributes,
                visibility,
                kind,
                span: start.through(end),
            });
        }
        if self.in_impl && !matches!(self.current().kind, K::Fn | K::Logic | K::Const) {
            return self.fail(
                "an `impl` block holds functions and constants: `fn`, `logic fn`, or `const`",
            );
        }
        if !self.declaration_start() {
            return self.fail(
                "expected a declaration: `fn`, `struct`, `enum`, `prop`, `const`, or `impl`",
            );
        }
        let (kind, end) = self.item(visibility.as_ref().map(|visibility| visibility.span))?;
        Ok(Declaration {
            doc,
            attributes,
            visibility,
            kind,
            span: start.through(end),
        })
    }

    /// The doc comments and attributes before an item, in any order. An
    /// inner attribute or doc comment here is reported and left out.
    #[inline(never)]
    fn outer_attributes(&mut self) -> ParseResult<(Vec<DocComment>, Vec<Attribute>)> {
        let mut doc = Vec::new();
        let mut attributes = Vec::new();
        while self.item_prefix_start() {
            self.step();
            if let Some(token) = self.eat(K::OuterDoc) {
                doc.push(self.doc_comment(token));
            } else if let Some(token) = self.eat(K::InnerDoc) {
                self.diagnostics.push(Diagnostic::error(
                    "L0121",
                    "an inner doc comment (`//!`) goes at the top of the file, before any item",
                    token.span,
                ));
            } else if let Some((attribute, inner)) = self.attribute()? {
                if inner {
                    self.diagnostics.push(Diagnostic::error(
                        "L0121",
                        "an inner attribute (`#![...]`) goes at the top of the file, before any item",
                        attribute.span,
                    ));
                } else {
                    attributes.push(attribute);
                }
            }
        }
        Ok((doc, attributes))
    }

    /// The doc comments before a field or a variant, which take no attribute.
    #[inline(never)]
    fn doc_comments(&mut self) -> ParseResult<Vec<DocComment>> {
        let (doc, attributes) = self.outer_attributes()?;
        for attribute in attributes {
            self.diagnostics.push(Diagnostic::error(
                "L0121",
                format!(
                    "`#[{}]` goes before an item; a field or a variant takes no attribute",
                    attribute.kind.name()
                ),
                attribute.span,
            ));
        }
        Ok(doc)
    }

    fn doc_comment(&self, token: Token) -> DocComment {
        DocComment {
            text: self.string(token),
            span: token.span,
        }
    }

    /// `//!` comments and `#![...]` attributes at the top of the file.
    #[inline(never)]
    fn file_header(&mut self, program: &mut Program) {
        loop {
            self.step();
            if let Some(token) = self.eat(K::InnerDoc) {
                program.doc.push(self.doc_comment(token));
            } else if self.at(K::Hash) && self.peek(1) == K::Bang && self.peek(2) == K::LBracket {
                match self.attribute() {
                    Ok(Some((attribute, _))) => program.attributes.push(attribute),
                    Ok(None) => {}
                    Err(()) => return,
                }
            } else {
                return;
            }
        }
    }

    /// `pub`, `pub(crate)`, `pub(super)`, `pub(self)`, or `pub(in path)`.
    #[inline(never)]
    fn visibility(&mut self) -> ParseResult<Option<Visibility>> {
        if !self.at_keyword("pub") {
            return Ok(None);
        }
        let start = self.bump();
        if !self.at(K::LParen) {
            return Ok(Some(Visibility {
                scope: VisibilityScope::Public,
                span: start.span,
            }));
        }
        let opening = self.bump();
        let scope = if self.at_keyword("crate") {
            self.bump();
            VisibilityScope::Crate
        } else if self.at_keyword("super") {
            self.bump();
            VisibilityScope::Super
        } else if self.at_keyword("self") {
            self.bump();
            VisibilityScope::SelfModule
        } else if self.eat(K::In).is_some() {
            if !self.at_named() {
                return self.fail("`pub(in ...)` names a module by its path");
            }
            VisibilityScope::In(self.path()?)
        } else {
            return self.fail(
                "`pub` is restricted with `pub(crate)`, `pub(super)`, `pub(self)`, or `pub(in path)`",
            );
        };
        let end = self.close(K::RParen, opening)?;
        Ok(Some(Visibility {
            scope,
            span: start.span.through(end.span),
        }))
    }

    /// `pub` before a name or a restriction, where it reads as visibility;
    /// `pub: u8` and `pub(u8)` are the keyword meant as a name.
    fn at_visibility(&self) -> bool {
        self.at_keyword("pub")
            && (self.peek(1) == K::Name
                || (self.peek(1) == K::LParen
                    && (self.peek(2) == K::In
                        || self.tokens.get(self.position + 2).is_some_and(|token| {
                            token.kind == K::Keyword
                                && matches!(
                                    self.source.slice(token.span),
                                    Some("crate" | "super" | "self")
                                )
                        }))))
    }

    /// L0122: `pub` where nothing can be made visible.
    #[inline(never)]
    fn no_visibility(&mut self, message: &str) -> ParseResult<()> {
        if self.at_visibility()
            && let Some(visibility) = self.visibility()?
        {
            // Reported and passed over: what follows is still read.
            self.diagnostics
                .push(Diagnostic::error("L0122", message, visibility.span));
        }
        Ok(())
    }

    /// One declaration form, from its keyword. Returns the kind and where it
    /// ends.
    fn item(&mut self, visible: Option<Span>) -> ParseResult<(DeclarationKind, Span)> {
        let start = self.bump();
        match start.kind {
            K::Fn => self.function(false),
            K::Logic => {
                self.expect(K::Fn)?;
                self.function(true)
            }
            K::Struct => {
                let name = self.name()?;
                let generics = self.generic_parameters()?;
                let (fields, end) = self.struct_fields()?;
                Ok((
                    DeclarationKind::Struct {
                        generics,
                        name,
                        fields,
                    },
                    end.span,
                ))
            }
            K::Enum => self.enum_declaration(),
            K::Const => {
                let name = self.name()?;
                self.expect(K::Colon)?;
                let ty = self.ty()?;
                self.expect(K::Equal)?;
                let value = self.expression()?;
                let end = self.semicolon(value.span, "constant declaration")?;
                Ok((DeclarationKind::Constant { name, ty, value }, end.span))
            }
            K::Keyword if self.source.slice(start.span) == Some("impl") => self.impl_block(visible),
            K::Keyword => self
                .fail("expected a declaration: `fn`, `struct`, `enum`, `prop`, `const`, or `impl`"),
            // `declaration_start` leaves the contextual word `prop`.
            _ => self.prop(),
        }
    }

    /// A checked header with a deliberately trusted native specification.
    fn trusted_function(&mut self) -> ParseResult<(DeclarationKind, Attribute, Span)> {
        let start = self.bump().span;
        if !self.at(K::String) {
            return self.fail("a trusted declaration needs a nonempty reason string before `fn`");
        }
        let token = self.bump();
        let reason = self.string(token);
        if reason.trim().is_empty() {
            return self.fail("a trusted declaration's reason cannot be empty");
        }
        self.expect(K::Fn)?;
        let name = self.name()?;
        let generics = self.generic_parameters()?;
        let (self_param, parameters) = self.parameter_list(true, false)?;
        self.expect(K::Arrow)?;
        let result = self.ty()?;
        self.expect(K::Equal)?;
        let implementation = self.path()?;
        let end = self
            .semicolon(implementation.span, "trusted declaration")?
            .span;
        let body = Block {
            statements: Vec::new(),
            tail: None,
            span: implementation.span,
        };
        let attribute = Attribute {
            kind: AttributeKind::Trusted {
                reason,
                implementation,
            },
            span: start.through(end),
        };
        Ok((
            DeclarationKind::Function {
                logical: false,
                generics,
                name,
                self_param,
                parameters,
                result,
                body,
            },
            attribute,
            end,
        ))
    }

    fn function(&mut self, logical: bool) -> ParseResult<(DeclarationKind, Span)> {
        let name = self.name()?;
        let generics = self.generic_parameters()?;
        // `self` is a name in the signature and the body of a method: a
        // parameter's type or the result type may speak of it (O4).
        let method = self.in_impl && self.at(K::LParen) && self.self_param_follows();
        let saved = std::mem::replace(&mut self.in_method, method);
        let signature = (|| {
            let (self_param, parameters) = self.parameter_list(true, self.in_impl)?;
            self.expect(K::Arrow)?;
            let result = self.ty()?;
            Ok((self_param, parameters, result))
        })();
        let body = match &signature {
            Ok(_) => self.block(),
            Err(()) => Err(()),
        };
        self.in_method = saved;
        let (self_param, parameters, result) = signature?;
        let body = body?;
        let end = body.span;
        Ok((
            DeclarationKind::Function {
                logical,
                generics,
                name,
                self_param,
                parameters,
                result,
                body,
            },
            end,
        ))
    }

    /// `impl Name { methods }`
    #[inline(never)]
    fn impl_block(&mut self, visible: Option<Span>) -> ParseResult<(DeclarationKind, Span)> {
        if let Some(span) = visible {
            self.diagnostics.push(Diagnostic::error(
                "L0122",
                "`pub` is not written on an `impl` block; write it on each method",
                span,
            ));
        }
        self.no_generics()?;
        if !self.at_named() {
            return self.fail("expected the type an `impl` block is for");
        }
        let first = self.path()?;
        if first.text() == "Model" && self.at(K::For) {
            self.bump();
            let source = self.ty()?;
            let opening = self.expect(K::LBrace)?;
            if self.source.slice(self.current().span) != Some("type") {
                return self.fail("a Model implementation starts with `type Logic = LogicalType;`");
            }
            self.bump();
            let associated = self.name()?;
            if associated.text != "Logic" {
                return self.fail("the Model associated type is named `Logic`");
            }
            self.expect(K::Equal)?;
            let target_type = self.ty()?;
            self.expect(K::Semicolon)?;
            let target = match &target_type.kind {
                TypeKind::Named(name) => Path {
                    segments: vec![name.clone()],
                    span: name.span,
                },
                TypeKind::Path { path, .. } => *path.clone(),
                _ => return self.fail("a Model destination is a named logical type"),
            };
            let saved = std::mem::replace(&mut self.in_impl, true);
            let methods = self.impl_body();
            self.in_impl = saved;
            let mut methods = methods?;
            let end = self.close(K::RBrace, opening)?;
            // The special trait lowers to a checked logical definition with
            // an explicit shared source argument. No new kernel rule.
            for method in &mut methods {
                if let DeclarationKind::Function {
                    self_param,
                    parameters,
                    ..
                } = &mut method.kind
                    && let Some(receiver) = self_param.take()
                {
                    if receiver.kind != SelfKind::Ref {
                        return self.fail("Model::model requires `&self`");
                    }
                    parameters.insert(
                        0,
                        Parameter {
                            name: Name {
                                text: "self".into(),
                                span: receiver.span,
                            },
                            mutable: false,
                            ty: Type {
                                span: receiver.span,
                                kind: TypeKind::Ref {
                                    lifetime: None,
                                    mutable: false,
                                    inner: Box::new(source.clone()),
                                },
                            },
                            span: receiver.span,
                        },
                    );
                }
            }
            let span = first.span.through(end.span);
            return Ok((
                DeclarationKind::Impl {
                    target,
                    model: Some(ModelImpl {
                        source,
                        target: target_type,
                        span,
                    }),
                    methods,
                },
                end.span,
            ));
        }
        if first.text() == "Model" && self.at(K::Less) {
            return self.fail("write `impl Model for Source { type Logic = Destination; logic fn model(&self) -> Self::Logic { ... } }`; each source has one canonical model");
        }
        let (target, model) = (first, None);
        self.no_generics()?;
        if self.at(K::For) {
            self.diagnostics.push(Diagnostic::error(
                "L0116",
                "traits (`impl Trait for Type`) are not in Locus yet",
                self.current().span,
            ));
            return Err(());
        }
        let opening = self.expect(K::LBrace)?;
        let saved = std::mem::replace(&mut self.in_impl, true);
        let methods = self.impl_body();
        self.in_impl = saved;
        let methods = methods?;
        let end = self.close(K::RBrace, opening)?;
        Ok((
            DeclarationKind::Impl {
                model,
                target,
                methods,
            },
            end.span,
        ))
    }

    /// The methods of an `impl` block, each recovered on its own.
    #[inline(never)]
    fn impl_body(&mut self) -> ParseResult<Vec<Declaration>> {
        let mut methods = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            let start = self.position;
            match self.declaration() {
                Ok(method) => methods.push(method),
                Err(()) => self.recover_declaration(),
            }
            if self.position == start {
                self.bump();
            }
        }
        Ok(methods)
    }

    #[inline(never)]
    fn generic_parameters(&mut self) -> ParseResult<Vec<GenericParameter>> {
        let Some(opening) = self.eat(K::Less) else {
            return Ok(Vec::new());
        };
        let mut parameters = Vec::new();
        // Preserve a focused declaration error for an invalid generic token.
        if self.at(K::Error) {
            self.diagnostics.push(Diagnostic::error(
                "L0116",
                "generic parameters are not in Locus yet",
                opening.span,
            ));
            return Err(());
        }
        if self.at_angle_close() {
            return self.fail("a generic parameter list cannot be empty");
        }
        while !self.at_angle_close() && !self.at(K::Eof) {
            self.step();
            let lifetime = self.at(K::Lifetime);
            let name = if lifetime {
                self.lifetime_name()?
            } else {
                self.name()?
            };
            let mut end = name.span;
            let mut bounds = Vec::new();
            if self.eat(K::Colon).is_some() {
                if lifetime {
                    return self.fail("lifetime bounds are not supported yet");
                }
                loop {
                    self.step();
                    if !self.at_named() {
                        return self.fail("expected a generic bound such as `Logical`");
                    }
                    let bound = self.path()?;
                    end = bound.span;
                    bounds.push(bound);
                    if self.eat(K::Plus).is_none() {
                        break;
                    }
                }
            }
            parameters.push(GenericParameter {
                lifetime,
                span: name.span.through(end),
                name,
                bounds,
            });
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        self.close_angle(opening)?;
        Ok(parameters)
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

    #[inline(never)]
    fn enum_declaration(&mut self) -> ParseResult<(DeclarationKind, Span)> {
        let name = self.name()?;
        let generics = self.generic_parameters()?;
        let opening = self.expect(K::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            let doc = self.doc_comments()?;
            self.no_visibility(
                "a variant is as visible as its enum, and `pub` is not written on it",
            )?;
            let name = self.name()?;
            let (shape, fields, end) = self.variant_fields(name.span)?;
            variants.push(Variant {
                span: name.span.through(end),
                doc,
                name,
                shape,
                fields,
            });
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(K::RBrace, opening)?;
        Ok((
            DeclarationKind::Enum {
                generics,
                name,
                variants,
            },
            end.span,
        ))
    }

    fn prop(&mut self) -> ParseResult<(DeclarationKind, Span)> {
        let name = self.name()?;
        let generics = self.generic_parameters()?;
        let parameters = if self.at(K::LParen) {
            self.parameters()?
        } else {
            Vec::new()
        };
        let opening = self.expect(K::LBrace)?;
        let mut variants = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            let doc = self.doc_comments()?;
            self.no_visibility(
                "a constructor is as visible as its proposition, and `pub` is not written on it",
            )?;
            let name = self.name()?;
            let (shape, fields, mut end) = self.variant_fields(name.span)?;
            let target = if self.eat(K::Colon).is_some() {
                let at = self.expect(K::At)?;
                let target = self.unrestricted(|parser| parser.proof_target(at))?;
                end = target.span;
                Some(target)
            } else {
                None
            };
            let body = if self.eat(K::Implies).is_some() {
                let body = self.block()?;
                end = body.span;
                Some(body)
            } else {
                None
            };
            let has_body = body.is_some();
            variants.push(PropVariant {
                span: name.span.through(end),
                doc,
                name,
                shape,
                fields,
                target,
                body,
            });
            if self.eat(K::Comma).is_none() && !has_body {
                break;
            }
        }
        let end = self.close(K::RBrace, opening)?;
        Ok((
            DeclarationKind::Prop {
                generics,
                name,
                parameters,
                variants,
            },
            end.span,
        ))
    }

    /// The optional `( fields )` or `{ name: Type, ... }` of a variant, and
    /// where the variant ends.
    fn variant_fields(&mut self, name: Span) -> ParseResult<(VariantShape, Vec<TypeField>, Span)> {
        if self.at(K::LParen) {
            let (fields, end) = self.type_fields()?;
            return Ok((VariantShape::Tuple, fields, end.span));
        }
        if self.at(K::LBrace) {
            let (fields, end) = self.named_type_fields()?;
            return Ok((VariantShape::Struct, fields, end.span));
        }
        Ok((VariantShape::Unit, Vec::new(), name))
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

    /// `{ name: Type, ... }`: the fields of a variant written with braces,
    /// each of which has a name.
    #[inline(never)]
    fn named_type_fields(&mut self) -> ParseResult<(Vec<TypeField>, Token)> {
        let opening = self.expect(K::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            if self.peek(1) != K::Colon {
                return self.fail("a field of a variant written with braces is `name: Type`");
            }
            fields.push(self.type_field()?);
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(K::RBrace, opening)?;
        Ok((fields, end))
    }

    /// `{ pub name: Type, ... }`: the fields of a struct.
    #[inline(never)]
    fn struct_fields(&mut self) -> ParseResult<(Vec<Field>, Token)> {
        let opening = self.expect(K::LBrace)?;
        let mut fields = Vec::new();
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            let doc = self.doc_comments()?;
            // `pub: u8` is the keyword meant as a field's name.
            let visibility = if self.peek(1) == K::Colon {
                None
            } else {
                self.visibility()?
            };
            let name = self.name()?;
            self.expect(K::Colon)?;
            let ty = self.ty()?;
            fields.push(Field {
                span: name.span.through(ty.span),
                doc,
                visibility,
                name,
                ty,
            });
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(K::RBrace, opening)?;
        Ok((fields, end))
    }

    /// The parameters of a quantifier or a proposition: names and types.
    fn parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        Ok(self.parameter_list(false, false)?.1)
    }

    /// `( self, name: Type, ... )`. A `self` parameter, in any of its four
    /// forms, comes first and only in a method; `mut` before a name is
    /// written on a function's parameter alone.
    fn parameter_list(
        &mut self,
        function: bool,
        method: bool,
    ) -> ParseResult<(Option<SelfParam>, Vec<Parameter>)> {
        let opening = self.expect(K::LParen)?;
        let mut self_param = None;
        if self.at_self_param() {
            self_param = Some(self.self_param(method)?);
            if self.eat(K::Comma).is_none() && !self.at(K::RParen) {
                return self.fail("expected `,` or `)` after the `self` parameter");
            }
        }
        let mut parameters = Vec::new();
        while !self.at(K::RParen) && !self.at(K::Eof) {
            self.step();
            if self.at_self_param() {
                self.diagnostics.push(Diagnostic::error(
                    "L0100",
                    "a `self` parameter comes first",
                    self.current().span,
                ));
                return Err(());
            }
            self.no_visibility("`pub` is not written on a parameter")?;
            let start = self.current().span;
            // `mut: u8` is the keyword meant as a name, which `name` reports.
            let mutable = self.at(K::Mut) && self.peek(1) != K::Colon;
            if mutable {
                if !function {
                    return self.misplaced_mut(
                        start,
                        "a parameter of a quantifier or a proposition never changes, and `mut` is not written on it",
                    );
                }
                self.bump();
            }
            let name = self.name()?;
            self.expect(K::Colon)?;
            let ty = self.ty()?;
            parameters.push(Parameter {
                span: start.through(ty.span),
                mutable,
                name,
                ty,
            });
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        self.close(K::RParen, opening)?;
        Ok((self_param, parameters))
    }

    /// Whether a `self` parameter opens the parameter list at the `(`.
    fn self_param_follows(&self) -> bool {
        let mut at = 1;
        if self.peek(at) == K::And {
            at += 1;
        }
        if self.peek(at) == K::Mut {
            at += 1;
        }
        self.tokens.get(self.position + at).is_some_and(|token| {
            token.kind == K::Keyword && self.source.slice(token.span) == Some("self")
        }) && self.peek(at + 1) != K::PathSep
    }

    /// `self`, `mut self`, `&self`, or `&mut self`, when not the start of
    /// a `self::` path.
    fn at_self_param(&self) -> bool {
        let mut at = 0;
        if self.peek(at) == K::And {
            at += 1;
        }
        if self.peek(at) == K::Mut {
            at += 1;
        }
        self.tokens.get(self.position + at).is_some_and(|token| {
            token.kind == K::Keyword && self.source.slice(token.span) == Some("self")
        }) && self.peek(at + 1) != K::PathSep
    }

    #[inline(never)]
    fn self_param(&mut self, method: bool) -> ParseResult<SelfParam> {
        let start = self.current();
        let reference = self.eat(K::And).is_some();
        let mutable = self.eat(K::Mut).is_some();
        let end = self.bump();
        let span = start.span.through(end.span);
        if !method {
            self.diagnostics.push(Diagnostic::error(
                "L0100",
                "a `self` parameter belongs to a method in an `impl` block",
                span,
            ));
            return Err(());
        }
        let kind = match (reference, mutable) {
            (false, false) => SelfKind::Value,
            (false, true) => SelfKind::MutValue,
            (true, false) => SelfKind::Ref,
            (true, true) => SelfKind::RefMut,
        };
        Ok(SelfParam { kind, span })
    }

    fn ty(&mut self) -> ParseResult<Type> {
        self.nested(Self::ty_inner)
    }

    fn ty_inner(&mut self) -> ParseResult<Type> {
        let start = self.current();
        match start.kind {
            K::Fn => self.function_type(start),
            K::Logic => self.logical_function_type(),
            K::Name | K::Keyword if self.at_named() => self.named_type(false),
            K::At => {
                self.bump();
                let proposition = self.proof_target(start)?;
                Ok(Type {
                    span: start.span.through(proposition.span),
                    kind: TypeKind::Proof(Box::new(proposition)),
                })
            }
            K::Hash => self.hash_syntax(),
            K::LBracket => self.collection_type(),
            K::And | K::AndAnd => self.reference_type(),
            K::Bang => {
                self.bump();
                Ok(Type {
                    span: start.span,
                    kind: TypeKind::Never,
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

    /// A type named by a name or a path, with type arguments or without:
    /// `u8`, `Self`, `a::B`, `Option<T>`. Directly after `as`, a `<` is
    /// the comparison that follows the cast.
    #[inline(never)]
    fn named_type(&mut self, after_as: bool) -> ParseResult<Type> {
        let mut path = self.path()?;
        if !self.at(K::Less) || (after_as && !self.cast_type_arguments_follow()?) {
            return Ok(match path.single() {
                Some(_) => Type {
                    span: path.span,
                    kind: TypeKind::Named(path.segments.pop().expect("one segment")),
                },
                None => Type {
                    span: path.span,
                    kind: TypeKind::Path {
                        path: Box::new(path),
                        arguments: Vec::new(),
                    },
                },
            });
        }
        let opening = self.bump();
        let mut arguments = Vec::new();
        while !self.at_angle_close() && !self.at(K::Eof) {
            self.step();
            arguments.push(self.type_argument()?);
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close_angle(opening)?;
        Ok(Type {
            span: path.span.through(end),
            kind: TypeKind::Path {
                path: Box::new(path),
                arguments,
            },
        })
    }

    // A closed generic argument list after a cast is a type application.
    // Without a closing angle before the next delimiter, `<` is comparison,
    // preserving `x as Int < 3`. Lookahead is bounded by parser's chain limit.
    fn cast_type_arguments_follow(&mut self) -> ParseResult<bool> {
        let mut angles = 0usize;
        for (index, token) in self
            .tokens
            .iter()
            .skip(self.position)
            .take(MAX_EXPRESSION_CHAIN + 1)
            .enumerate()
        {
            if index == MAX_EXPRESSION_CHAIN {
                return self.chain_limit();
            }
            match token.kind {
                K::Less => angles += 1,
                K::Greater => {
                    angles = angles.saturating_sub(1);
                    if angles == 0 {
                        return Ok(true);
                    }
                }
                K::ShiftRight => {
                    angles = angles.saturating_sub(2);
                    if angles == 0 {
                        return Ok(true);
                    }
                }
                K::Eof | K::Semicolon | K::Equal | K::LBrace | K::RBrace => return Ok(false),
                _ => {}
            }
        }
        Ok(false)
    }

    fn lifetime_name(&mut self) -> ParseResult<Name> {
        let token = self.expect(K::Lifetime)?;
        Ok(Name {
            text: self.source.slice(token.span).unwrap_or_default().into(),
            span: token.span,
        })
    }

    fn type_argument(&mut self) -> ParseResult<Type> {
        if self.at(K::Lifetime) {
            let name = self.lifetime_name()?;
            Ok(Type {
                span: name.span,
                kind: TypeKind::Lifetime(name),
            })
        } else {
            self.ty()
        }
    }

    /// `&'a T` or `&'a mut T`; elided lifetimes remain absent in the AST.
    #[inline(never)]
    fn reference_type(&mut self) -> ParseResult<Type> {
        let start = self.bump();
        let lifetime = if start.kind == K::And && self.at(K::Lifetime) {
            Some(self.lifetime_name()?)
        } else {
            None
        };
        let mutable = start.kind == K::And && self.eat(K::Mut).is_some();
        let inner = if start.kind == K::AndAnd {
            let span = Span::new(start.span.file, start.span.start + 1, start.span.end);
            let lifetime = if self.at(K::Lifetime) {
                Some(self.lifetime_name()?)
            } else {
                None
            };
            let mutable = self.eat(K::Mut).is_some();
            let inner = self.ty()?;
            Type {
                span: span.through(inner.span),
                kind: TypeKind::Ref {
                    lifetime,
                    mutable,
                    inner: Box::new(inner),
                },
            }
        } else {
            self.ty()?
        };
        Ok(Type {
            span: start.span.through(inner.span),
            kind: TypeKind::Ref {
                lifetime,
                mutable,
                inner: Box::new(inner),
            },
        })
    }

    fn at_angle_close(&self) -> bool {
        matches!(
            self.current().kind,
            K::Greater | K::ShiftRight | K::GreaterEqual | K::ShiftRightEqual
        )
    }

    /// The `>` that closes type arguments. The lexer reads `>>` as one
    /// token, so the second `>` of `Ghost<Option<T>>` is given back to the
    /// token stream, and likewise the `=` of `>=`.
    #[inline(never)]
    fn close_angle(&mut self, opening: Token) -> ParseResult<Span> {
        let token = self.current();
        let rest = match token.kind {
            K::Greater => None,
            K::ShiftRight => Some(K::Greater),
            K::GreaterEqual => Some(K::Equal),
            K::ShiftRightEqual => Some(K::GreaterEqual),
            _ => {
                self.close(K::Greater, opening)?;
                unreachable!("`close` fails on any other token")
            }
        };
        let Some(rest) = rest else {
            return Ok(self.bump().span);
        };
        let split = token.span.start + 1;
        self.tokens[self.position] = Token {
            kind: rest,
            span: Span::new(token.span.file, split, token.span.end),
            literal: 0,
        };
        Ok(Span::new(token.span.file, token.span.start, split))
    }

    #[inline(never)]
    fn no_type<T>(&mut self) -> ParseResult<T> {
        if self.current().kind.is_keyword() {
            return self.keyword_as_name();
        }
        if self.at_doc_comment() {
            return self.misplaced_doc_comment();
        }
        self.fail("expected a type such as `u8`, `Prop`, a tuple, or `@(condition)`")
    }

    fn at_doc_comment(&self) -> bool {
        matches!(self.current().kind, K::OuterDoc | K::InnerDoc)
    }

    #[inline(never)]
    fn misplaced_doc_comment<T>(&mut self) -> ParseResult<T> {
        self.fail("a doc comment goes before an item, a field, or a variant")
    }

    /// The proposition of a proof type: a name, which calls and projections
    /// may follow, or a formula in parentheses. A proof type is often
    /// followed by a block, so `claim {` is not a struct literal.
    fn proof_target(&mut self, at: Token) -> ParseResult<Expr> {
        match self.current().kind {
            _ if self.at_named() => self.header(false, |parser| parser.expression_bp(OPERAND)),
            K::LParen => self.formula_mode(Self::parenthesized),
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "L0100",
                    "a proof type needs a proposition: `@claim` or `@(condition)`",
                    at.span,
                ));
                Err(())
            }
        }
    }

    fn function_type(&mut self, start: Token) -> ParseResult<Type> {
        self.expect(K::Fn)?;
        let (parameters, _) = self.type_fields()?;
        self.expect(K::Arrow)?;
        let result = self.ty()?;
        Ok(Type {
            span: start.span.through(result.span),
            kind: TypeKind::Function {
                parameters,
                result: Box::new(result),
            },
        })
    }

    /// The capitalized `Fn` is a name, not the `fn` item keyword.
    #[inline(never)]
    fn logical_function_type(&mut self) -> ParseResult<Type> {
        let start = self.expect(K::Logic)?;
        if !self.at_word("Fn") {
            return self.fail("expected `Fn` after `logic` in a callable type");
        }
        self.bump();
        let (parameters, _) = self.type_fields()?;
        self.expect(K::Arrow)?;
        let result = self.ty()?;
        Ok(Type {
            span: start.span.through(result.span),
            kind: TypeKind::LogicalFunction {
                parameters,
                result: Box::new(result),
            },
        })
    }

    fn collection_type(&mut self) -> ParseResult<Type> {
        let start = self.expect(K::LBracket)?;
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
            kind,
            span: start.span.through(end.span),
        })
    }

    /// A keyword before `:` was meant as a name, and `name` says so.
    fn at_name_or_keyword(&self) -> bool {
        self.at(K::Name) || self.current().kind.is_keyword()
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
        self.nested(Self::pattern_with_at)
    }

    #[inline(never)]
    fn pattern_with_at(&mut self) -> ParseResult<Pattern> {
        let left = self.pattern_inner()?;
        let Some(at) = self.eat(K::At) else {
            return Ok(left);
        };
        let right = self.nested(Self::pattern_inner)?;
        if self.at(K::At) {
            return self.chained_at(at.span);
        }
        let span = left.span.through(right.span);
        let kind = match left.kind {
            PatternKind::Name { name, mutable } => PatternKind::Binding {
                name,
                mutable,
                pattern: Box::new(right),
                at_span: at.span,
            },
            PatternKind::Variant { .. } | PatternKind::Struct { .. } => PatternKind::Evidence {
                constructor: Box::new(left),
                evidence: Box::new(right),
                at_span: at.span,
            },
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    "L0151",
                    "the left of `@` must be a binding name or a proposition constructor",
                    left.span,
                ));
                return Err(());
            }
        };
        Ok(Pattern { kind, span })
    }

    fn pattern_inner(&mut self) -> ParseResult<Pattern> {
        let start = self.current();
        match start.kind {
            K::Name | K::Keyword if self.at_named() => self.named_pattern(),
            K::Mut => self.mut_pattern(),
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

    /// `mut name`: a binding the body may assign. Before a pattern that is
    /// not a name, `mut` is misplaced, in rustc's words; before a keyword,
    /// the keyword was meant as the name; before anything else, `mut` was.
    #[inline(never)]
    fn mut_pattern(&mut self) -> ParseResult<Pattern> {
        let start = self.bump();
        if self.at(K::Mut) {
            if self.peek(1) == K::Name {
                return self.misplaced_mut(start.span, "`mut` on a binding may not be repeated");
            }
            // The second `mut` was meant as the name.
            return self.mut_pattern();
        }
        if self.at(K::Name) || self.current().kind.is_keyword() {
            let name = self.name()?;
            return Ok(Pattern {
                span: start.span.through(name.span),
                kind: PatternKind::Name {
                    name,
                    mutable: true,
                },
            });
        }
        if self.at_pattern_start() {
            return self.misplaced_mut(
                start.span,
                "`mut` must be attached to each individual binding",
            );
        }
        self.diagnostics.push(keyword_is_no_name("mut", start.span));
        Err(())
    }

    /// Whether the current token can begin a pattern.
    fn at_pattern_start(&self) -> bool {
        self.at_named()
            || matches!(
                self.current().kind,
                K::True | K::False | K::Integer | K::Underscore | K::LParen
            )
    }

    /// L0125: `mut` where nothing can be made mutable, reported at the `mut`.
    #[inline(never)]
    fn misplaced_mut<T>(&mut self, span: Span, message: &str) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error("L0125", message, span)
                .note("`mut` goes before the name of a binding or of a parameter, as in `let (mut a, b) = pair;` or `fn f(mut n: u8)`"),
        );
        Err(())
    }

    #[inline(never)]
    fn no_pattern<T>(&mut self) -> ParseResult<T> {
        if self.current().kind.is_keyword() {
            return self.keyword_as_name();
        }
        if self.at_doc_comment() {
            return self.misplaced_doc_comment();
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

    /// A pattern that begins with a name or a path: a binding, a struct
    /// pattern, or a variant with or without its fields. A name before `(`
    /// is a variant, as `Some(x)` is in Rust.
    #[inline(never)]
    fn named_pattern(&mut self) -> ParseResult<Pattern> {
        let mut path = self.path()?;
        if self.at(K::LBrace) {
            return self.struct_pattern(path);
        }
        if path.single().is_some() && !self.at(K::LParen) {
            return Ok(Pattern {
                span: path.span,
                kind: PatternKind::Name {
                    name: path.segments.pop().expect("one segment"),
                    mutable: false,
                },
            });
        }
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

    /// `{ name: pattern, name, .. }` after a struct's name or a variant's path.
    #[inline(never)]
    fn struct_pattern(&mut self, path: Path) -> ParseResult<Pattern> {
        let opening = self.expect(K::LBrace)?;
        let mut fields = Vec::new();
        let mut rest = None;
        while !self.at(K::RBrace) && !self.at(K::Eof) {
            self.step();
            if let Some(dots) = self.eat(K::DotDot) {
                rest = Some(dots.span);
                break;
            }
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
            span: path.span.through(end.span),
            kind: PatternKind::Struct {
                path: Box::new(path),
                fields,
                rest,
            },
        })
    }

    /// `a::b::c` from a token that `at_named` accepted: one segment or more.
    fn path(&mut self) -> ParseResult<Path> {
        let first = self.first_segment()?;
        let mut span = first.span;
        let mut segments = vec![first];
        while self.at(K::PathSep) && self.peek(1) != K::Less {
            self.bump();
            self.step();
            let segment = self.name()?;
            span = span.through(segment.span);
            segments.push(segment);
        }
        Ok(Path { segments, span })
    }

    /// The first segment of a path: a name, `Self` inside an `impl`, or
    /// `crate`, `super`, or `self` before `::`.
    #[inline(never)]
    fn first_segment(&mut self) -> ParseResult<Name> {
        if self.at(K::Name) {
            return self.name();
        }
        let token = self.bump();
        let text = self.source.slice(token.span).unwrap_or_default().to_owned();
        if text == "Self" && !self.in_impl {
            self.diagnostics.push(Diagnostic::error(
                "L0100",
                "`Self` is the type of an `impl` block, and this is outside one",
                token.span,
            ));
            return Err(());
        }
        Ok(Name {
            text,
            span: token.span,
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
                match self.statement_expression() {
                    Ok(expression) if self.at(K::Equal) => self
                        .assignment(expression)
                        .map(|statement| statements.push(statement)),
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
                        } else if expression.is_block_like() {
                            // As in Rust, `if`, `match`, a loop, or a block
                            // is a statement on its own without a `;`.
                            statements.push(Statement {
                                span: expression.span,
                                kind: StatementKind::Expression(expression),
                            });
                            Ok(())
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

    /// The expression that begins a statement: `=` after it is an
    /// assignment, and one that ends in a block is the whole statement.
    fn statement_expression(&mut self) -> ParseResult<Expr> {
        self.statement = true;
        let result = self.expression();
        self.statement = false;
        result
    }

    #[inline(never)]
    fn let_statement(&mut self) -> ParseResult<Statement> {
        let opening = self.expect(K::Let)?;
        let mutable = self.at(K::Mut);
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
                mutable,
                pattern,
                annotation,
                value,
            },
        })
    }

    /// `place = value;` after its place has been read. L0123 for a place
    /// that is neither a name nor a field path, in rustc's words.
    #[inline(never)]
    fn assignment(&mut self, place: Expr) -> ParseResult<Statement> {
        let equal = self.expect(K::Equal)?;
        if !is_place(&place) {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0123",
                    "invalid left-hand side of assignment",
                    equal.span,
                )
                .label(place.span, "cannot assign to this expression")
                .note("the left-hand side of an assignment is a variable or a field of one, as in `total = e` or `lock.failures = e`"),
            );
            return Err(());
        }
        let value = self.expression()?;
        let closing = self.semicolon(value.span, "assignment")?;
        Ok(Statement {
            span: place.span.through(closing.span),
            kind: StatementKind::Assign { place, value },
        })
    }

    /// L0124: `=` after an expression that is not a statement's.
    #[inline(never)]
    fn assignment_in_value_position<T>(&mut self) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error(
                "L0124",
                "assignment is a statement and has no value",
                self.current().span,
            )
            .note("write the assignment on a line of its own, `place = value;`; to compare two values, write `==`"),
        );
        Err(())
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
    // This loop moves an `Expr` at one site only.
    fn expression_bp_inner(&mut self, minimum: u8) -> ParseResult<Expr> {
        // Taken here, so that no expression inside this one sees it.
        let statement = std::mem::take(&mut self.statement);
        let mut left = self.prefix()?;
        let mut chain = 0;
        loop {
            self.step();
            if chain >= MAX_EXPRESSION_CHAIN {
                return self.chain_limit();
            }
            chain += 1;
            // In statement position an expression that ends in a block is
            // complete, as in Rust: only `.` continues it, and `match x { }
            // (y)` is the match and then a tuple.
            let complete = statement && left.is_block_like();
            match self.operator(minimum, statement, complete)? {
                Some(operator) => left = self.extend(left, operator)?,
                None => break,
            }
        }
        Ok(left)
    }

    /// What follows an operand at the minimum binding power in force, when
    /// something does: a postfix operator, `as`, or a binary operator that
    /// binds at least as tightly as the minimum. Whatever else of Rust
    /// stands here and is not yet an operator of Locus is reported.
    #[inline(never)]
    fn operator(
        &mut self,
        minimum: u8,
        statement: bool,
        complete: bool,
    ) -> ParseResult<Option<Operator>> {
        let kind = self.current().kind;
        if complete && kind != K::Dot {
            return Ok(None);
        }
        match kind {
            K::LParen if minimum <= OPERAND => return Ok(Some(Operator::Call)),
            K::LBracket if minimum <= OPERAND => return Ok(Some(Operator::Subscript)),
            K::Dot if minimum <= OPERAND => return Ok(Some(Operator::Member)),
            K::As => return Ok((CAST >= minimum).then_some(Operator::Cast)),
            K::At => return Ok((minimum == EVIDENCE).then_some(Operator::Evidence)),
            // After a whole expression, `=` can only be an assignment, whose
            // place the statement has just read. After an operand it may end
            // the proposition of a proof type, as in `bounded:
            // @within_limit(n) = evidence`.
            K::Equal if minimum == 0 => {
                if statement {
                    return Ok(None);
                }
                return self.assignment_in_value_position();
            }
            // In the header of a `for`, `..` and `..=` end a bound.
            K::DotDot | K::DotDotEqual if self.for_header => return Ok(None),
            _ => {}
        }
        let Some((operator, (left_bp, right_bp))) = binary(kind) else {
            if self.rust_only() {
                return Err(());
            }
            return Ok(None);
        };
        if operator == BinaryOp::Implies && !self.formula {
            self.outside_formula("`=>` is implication only inside a formula");
            return Err(());
        }
        Ok((left_bp >= minimum).then_some(Operator::Binary(operator, right_bp)))
    }

    #[inline(never)]
    fn extend(&mut self, left: Expr, operator: Operator) -> ParseResult<Expr> {
        match operator {
            Operator::Call => self.call(left),
            Operator::Subscript => self.subscript(left),
            Operator::Member => self.member(left),
            Operator::Cast => self.cast(left),
            Operator::Evidence => self.evidence(left),
            Operator::Binary(operator, right_bp) => self.binary(left, operator, right_bp),
        }
    }

    #[inline(never)]
    fn evidence(&mut self, constructor: Expr) -> ParseResult<Expr> {
        let at = self.expect(K::At)?;
        if matches!(constructor.kind, ExprKind::Evidence { .. }) {
            return self.chained_at(at.span);
        }
        let evidence = self.expression_bp(EVIDENCE + 1)?;
        if self.at(K::At) {
            return self.chained_at(at.span);
        }
        Ok(Expr {
            span: constructor.span.through(evidence.span),
            kind: ExprKind::Evidence {
                constructor: Box::new(constructor),
                evidence: Box::new(evidence),
                at_span: at.span,
            },
        })
    }

    #[inline(never)]
    fn chained_at<T>(&mut self, first: Span) -> ParseResult<T> {
        self.diagnostics.push(Diagnostic::error("L0150", "`@` cannot be chained without parentheses", self.current().span)
            .label(first, "the first `@` is here")
            .note("parenthesize the nested construction or pattern, as in `Outer::Arm @ (Inner::Arm @ h)` or `whole @ (Pred::Arm @ h)`"));
        Err(())
    }

    /// L0119: `=>` or a quantifier where no formula is being read.
    #[inline(never)]
    fn outside_formula(&mut self, message: &str) {
        self.diagnostics.push(
            Diagnostic::error("L0119", message, self.current().span).note(
                "a formula is written inside `prop!(...)`, `prove!(...)`, or `@(...)`; in a `match`, `=>` follows an arm's pattern",
            ),
        );
    }

    #[inline(never)]
    fn chain_limit<T>(&mut self) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error(
                "L0108",
                "expression chain exceeds the parser limit",
                self.current().span,
            )
            .note(format!("MAX_EXPRESSION_CHAIN limit of {MAX_EXPRESSION_CHAIN}; split this expression using local bindings")),
        );
        Err(())
    }

    /// `left as Type`
    #[inline(never)]
    fn cast(&mut self, expr: Expr) -> ParseResult<Expr> {
        let token = self.expect(K::As)?;
        // Directly after `as`, a `<` is the comparison that follows the cast.
        let ty = if self.at_named() {
            self.nested(|parser| parser.named_type(true))?
        } else {
            self.ty()?
        };
        Ok(Expr {
            span: expr.span.through(ty.span),
            kind: ExprKind::Cast {
                source_hint: None,
                expr: Box::new(expr),
                as_span: token.span,
                ty,
            },
        })
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
            && let ExprKind::Binary {
                operator: previous,
                operator_span,
                ..
            } = &left.kind
            && previous.is_comparison()
        {
            return self.chained_comparison(*operator_span);
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
            K::Name | K::Prop if self.at_form() => self.form(),
            K::Logic => self.logic_expression(),
            K::Or | K::OrOr => self.closure_expression(),
            K::Name if self.at_quantifier() => self.quantifier(),
            K::Name | K::Keyword if self.at_named() => self.named(),
            K::Keyword if self.at_keyword("self") => self.self_expression(),
            K::Star => self.dereference(),
            K::Integer | K::String | K::True | K::False | K::Underscore | K::Error => self.atom(),
            K::OuterDoc | K::InnerDoc => self.misplaced_doc_comment(),
            K::Bang => self.not(),
            K::Minus => self.negate(),
            K::And | K::AndAnd => self.reference(),
            K::LParen => self.parenthesized(),
            K::LBracket => self.array_expression(),
            K::LBrace => self.block_expression(),
            K::If => self.if_expression(),
            K::Match => self.match_expression(),
            K::Loop => self.loop_expression(),
            K::While => self.while_expression(),
            K::For => self.for_expression(),
            K::Break => self.break_expression(),
            K::Continue => self.continue_expression(),
            K::Return => self.return_expression(),
            K::At => self.proof(),
            K::Hash => self.hash_syntax(),
            _ => self.fail("expected an expression"),
        }
    }

    fn array_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::LBracket)?;
        let mut elements = Vec::new();
        while !self.at(K::RBracket) && !self.at(K::Eof) {
            self.step();
            elements.push(self.expression()?);
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        let end = self.close(K::RBracket, start)?;
        Ok(Expr {
            kind: ExprKind::Array(elements),
            span: start.span.through(end.span),
        })
    }

    fn subscript(&mut self, value: Expr) -> ParseResult<Expr> {
        let start = self.expect(K::LBracket)?;
        let index = self.expression()?;
        let end = self.close(K::RBracket, start)?;
        Ok(Expr {
            span: value.span.through(end.span),
            kind: ExprKind::Subscript {
                value: Box::new(value),
                index: Box::new(index),
            },
        })
    }

    #[inline(never)]
    fn closure_expression(&mut self) -> ParseResult<Expr> {
        let opening = self.bump();
        let mut parameters = Vec::new();
        if opening.kind == K::Or {
            while !self.at(K::Or) && !self.at(K::Eof) {
                self.step();
                let name = self.name()?;
                self.expect(K::Colon)?;
                let ty = self.ty()?;
                parameters.push(Parameter {
                    mutable: false,
                    span: name.span.through(ty.span),
                    name,
                    ty,
                });
                if self.eat(K::Comma).is_none() {
                    break;
                }
            }
            self.expect(K::Or)?;
        }
        let body = self.unrestricted(Self::expression)?;
        Ok(Expr {
            span: opening.span.through(body.span),
            kind: ExprKind::Closure {
                parameters,
                body: Box::new(body),
            },
        })
    }

    /// `name!(...)`: a built-in form, whose name is one of a closed list.
    #[inline(never)]
    fn form(&mut self) -> ParseResult<Expr> {
        let name = self.bump();
        let bang = self.bump();
        let spelling = self.source.slice(name.span).unwrap_or_default();
        let Some(form) = Form::from_name(spelling) else {
            return self.unknown_form(name.span.through(bang.span), spelling);
        };
        if !self.at(K::LParen) {
            return self.fail_form(
                name.span.through(self.current().span),
                format!("Locus forms take parentheses: `{spelling}!(...)`"),
            );
        }
        let (arguments, closing) = if form.takes_formula() {
            self.formula_argument(form)?
        } else {
            self.arguments()?
        };
        Ok(Expr {
            span: name.span.through(closing.span),
            kind: ExprKind::Form {
                form,
                name_span: name.span,
                arguments,
                source_hint: None,
            },
        })
    }

    /// The one formula of `prop!(...)` or `prove!(...)`.
    #[inline(never)]
    fn formula_argument(&mut self, form: Form) -> ParseResult<(Vec<Expr>, Token)> {
        let opening = self.expect(K::LParen)?;
        if self.at(K::RParen) || self.at(K::Comma) {
            return self.fail_form(
                self.current().span,
                format!("`{}!` takes one formula", form.name()),
            );
        }
        let formula = self.formula_mode(|parser| parser.unrestricted(Self::expression))?;
        if self.at(K::Comma) {
            return self.fail_form(
                self.current().span,
                format!("`{}!` takes one formula, and this is a second", form.name()),
            );
        }
        let closing = self.close(K::RParen, opening)?;
        Ok((vec![formula], closing))
    }

    #[inline(never)]
    fn fail_form<T>(&mut self, span: Span, message: String) -> ParseResult<T> {
        self.diagnostics
            .push(Diagnostic::error("L0118", message, span));
        Err(())
    }

    #[inline(never)]
    fn unknown_form<T>(&mut self, span: Span, spelling: &str) -> ParseResult<T> {
        let list: Vec<String> = Form::ALL
            .iter()
            .map(|form| format!("`{}!`", form.name()))
            .collect();
        let (last, rest) = list.split_last().expect("the list of forms is not empty");
        self.diagnostics.push(
            Diagnostic::error(
                "L0118",
                format!("`{spelling}!` is not a form of Locus"),
                span,
            )
            .note(format!(
                "the forms are {}, and {last}; Locus has no user-defined macros",
                rest.join(", ")
            )),
        );
        Err(())
    }

    /// An expression that begins with a name or a path: the name, the path,
    /// or a struct literal where one can stand.
    #[inline(never)]
    fn named(&mut self) -> ParseResult<Expr> {
        let mut path = self.path()?;
        let arguments = if self.at(K::PathSep) && self.peek(1) == K::Less {
            self.bump();
            let opening = self.expect(K::Less)?;
            let mut arguments = Vec::new();
            if self.at_angle_close() {
                return self.fail("a type argument list cannot be empty");
            }
            while !self.at_angle_close() && !self.at(K::Eof) {
                self.step();
                arguments.push(self.type_argument()?);
                if self.eat(K::Comma).is_none() {
                    break;
                }
            }
            let end = self.close_angle(opening)?;
            path.span = path.span.through(end);
            while self.eat(K::PathSep).is_some() {
                self.step();
                let segment = self.name()?;
                path.span = path.span.through(segment.span);
                path.segments.push(segment);
            }
            Some(arguments)
        } else {
            None
        };
        let callee = if self.at(K::LBrace) && !self.no_struct {
            self.struct_literal(path)?
        } else {
            Expr {
                span: path.span,
                kind: match path.single() {
                    Some(_) => ExprKind::Name(path.segments.pop().expect("one segment")),
                    None => ExprKind::Path(Box::new(path)),
                },
            }
        };
        Ok(match arguments {
            Some(arguments) => Expr {
                span: callee.span,
                kind: ExprKind::GenericApply {
                    callee: Box::new(callee),
                    arguments,
                },
            },
            None => callee,
        })
    }

    /// `self`, the receiver, in the body of a method that has one.
    #[inline(never)]
    fn self_expression(&mut self) -> ParseResult<Expr> {
        let token = self.current();
        if !self.in_method {
            self.diagnostics.push(Diagnostic::error(
                "L0100",
                "`self` is the receiver of a method, and this function has no `self` parameter",
                token.span,
            ));
            return Err(());
        }
        self.bump();
        Ok(Expr {
            span: token.span,
            kind: ExprKind::Name(Name {
                text: "self".into(),
                span: token.span,
            }),
        })
    }

    #[inline(never)]
    fn atom(&mut self) -> ParseResult<Expr> {
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

    /// L0103, as rustc words it: `a < b < c` and `a == b == c` are errors,
    /// not left-associative.
    #[inline(never)]
    fn chained_comparison<T>(&mut self, first: Span) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error(
                "L0103",
                "comparison operators cannot be chained",
                self.current().span,
            )
            .label(first, "the first comparison is here")
            .note("parenthesize the comparison that is an operand, as in `(a == b) == c`, or join two comparisons with `&&`, as in `a < b && b < c`"),
        );
        Err(())
    }

    #[inline(never)]
    fn not(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        let value = self.expression_bp(OPERAND)?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Not(Box::new(value)),
        })
    }

    /// Prefix dereference; the type checker determines the referent and permissions.
    #[inline(never)]
    fn dereference(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        let value = self.expression_bp(OPERAND)?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Unary {
                operator: UnaryOp::Deref,
                operator_span: start.span,
                expr: Box::new(value),
            },
        })
    }

    /// `-value`
    #[inline(never)]
    fn negate(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        let value = self.expression_bp(OPERAND)?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Unary {
                operator: UnaryOp::Neg,
                operator_span: start.span,
                expr: Box::new(value),
            },
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
    fn logic_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::Logic)?;
        let block = self.block()?;
        Ok(Expr {
            span: start.span.through(block.span),
            kind: ExprKind::Logic(block),
        })
    }

    /// Whether `break` or `return` stands alone: what follows cannot begin
    /// an expression.
    fn at_end_of_jump(&self) -> bool {
        matches!(
            self.current().kind,
            K::Semicolon | K::RBrace | K::RParen | K::RBracket | K::Comma | K::Eof
        )
    }

    /// `break`, or `break value`.
    #[inline(never)]
    fn break_expression(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        if self.at_end_of_jump() {
            return Ok(Expr {
                span: start.span,
                kind: ExprKind::Break(None),
            });
        }
        let value = self.expression()?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Break(Some(Box::new(value))),
        })
    }

    /// `continue`, which carries nothing: the next pass of the loop reads
    /// the `let mut` bindings as they are.
    #[inline(never)]
    fn continue_expression(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        if self.at(K::LParen) {
            return self
                .fail("`continue` carries nothing; a loop's state is its `let mut` bindings");
        }
        Ok(Expr {
            span: start.span,
            kind: ExprKind::Continue,
        })
    }

    /// `return`, or `return value`.
    #[inline(never)]
    fn return_expression(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        if self.at_end_of_jump() {
            return Ok(Expr {
                span: start.span,
                kind: ExprKind::Return(None),
            });
        }
        let value = self.expression()?;
        Ok(Expr {
            span: start.span.through(value.span),
            kind: ExprKind::Return(Some(Box::new(value))),
        })
    }

    /// `&value` or `&mut value`, binding as the other prefix operators do.
    /// The lexer reads `&&` as one token, so `&&value` is a reference to a
    /// reference, as it is in Rust.
    #[inline(never)]
    fn reference(&mut self) -> ParseResult<Expr> {
        let start = self.bump();
        let mutable = start.kind == K::And && self.eat(K::Mut).is_some();
        let inner = if start.kind == K::AndAnd {
            let span = Span::new(start.span.file, start.span.start + 1, start.span.end);
            let mutable = self.eat(K::Mut).is_some();
            let value = self.expression_bp(OPERAND)?;
            Expr {
                span: span.through(value.span),
                kind: ExprKind::Ref {
                    mutable,
                    expr: Box::new(value),
                },
            }
        } else {
            self.expression_bp(OPERAND)?
        };
        Ok(Expr {
            span: start.span.through(inner.span),
            kind: ExprKind::Ref {
                mutable,
                expr: Box::new(inner),
            },
        })
    }

    #[inline(never)]
    fn quantifier(&mut self) -> ParseResult<Expr> {
        let universal = self.at_word("forall");
        let word = if universal { "forall" } else { "exists" };
        if !self.formula {
            self.outside_formula(&format!(
                "`{word} (...)` is a quantifier only inside a formula"
            ));
            return Err(());
        }
        let start = self.bump();
        let parameters = self.parameters()?;
        if parameters.is_empty() {
            return self.fail(format!("`{word}` needs at least one parameter"));
        }
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: if universal {
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
        self.diagnostics.push(
            Diagnostic::error(
                "L0110",
                "`@` begins a proof type, not an expression",
                start.span,
            )
            .note("`@claim` and `@(condition)` are types; evidence is an ordinary expression: request it with `let evidence: @claim = _;`, or state a claim where it stands with `prove!(condition)`"),
        );
        Err(())
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

    /// `{ name: value, name }` after a struct's name or a variant's path.
    #[inline(never)]
    fn struct_literal(&mut self, path: Path) -> ParseResult<Expr> {
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
            span: path.span.through(end.span),
            kind: ExprKind::Struct { path, fields },
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

    /// `loop { ... }`.
    #[inline(never)]
    fn loop_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::Loop)?;
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: ExprKind::Loop { body },
        })
    }

    /// `while condition { ... }` or `while let pattern = value { ... }`. As
    /// in the header of an `if`, a struct literal cannot stand in the
    /// condition.
    #[inline(never)]
    fn while_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::While)?;
        let pattern = if self.eat(K::Let).is_some() {
            let pattern = self.pattern()?;
            self.expect(K::Equal)?;
            Some(Box::new(pattern))
        } else {
            None
        };
        let condition = self.header(false, Self::expression)?;
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: ExprKind::While {
                pattern,
                condition: Box::new(condition),
                body,
            },
        })
    }

    /// `for pattern in lower..upper { ... }`, with `..=` for an inclusive
    /// range, or `for pattern in value { ... }` over anything else.
    #[inline(never)]
    fn for_expression(&mut self) -> ParseResult<Expr> {
        let start = self.expect(K::For)?;
        // `for mut in` is the keyword meant as the index.
        if self.at(K::Mut) && self.peek(1) == K::In {
            return self.keyword_as_name();
        }
        let pattern = self.pattern()?;
        self.expect(K::In)?;
        let iterable = self.header(true, Self::for_iterable)?;
        let body = self.block()?;
        Ok(Expr {
            span: start.span.through(body.span),
            kind: ExprKind::For {
                pattern: Box::new(pattern),
                iterable: Box::new(iterable),
                body,
            },
        })
    }

    /// The range or other value a `for` runs over.
    #[inline(never)]
    fn for_iterable(&mut self) -> ParseResult<Expr> {
        let lower = self.expression()?;
        let kind = match self.current().kind {
            K::DotDot => RangeKind::Exclusive,
            K::DotDotEqual => RangeKind::Inclusive,
            _ => return Ok(lower),
        };
        self.bump();
        let upper = self.expression()?;
        Ok(Expr {
            span: lower.span.through(upper.span),
            kind: ExprKind::Range {
                kind,
                lower: Box::new(lower),
                upper: Box::new(upper),
            },
        })
    }

    fn hash_syntax<T>(&mut self) -> ParseResult<T> {
        if self.attribute_start() {
            self.misplaced_attribute();
            return Err(());
        }
        self.diagnostics.push(
            Diagnostic::error(
                "L0111",
                "`#` begins an attribute, `#[name]`, and nothing else",
                self.current().span,
            )
            .note("proof types are `@claim` or `@(condition)`, and `_` asks the elaborator for evidence"),
        );
        Err(())
    }

    fn attribute_start(&self) -> bool {
        self.at(K::Hash)
            && (self.peek(1) == K::LBracket
                || (self.peek(1) == K::Bang && self.peek(2) == K::LBracket))
    }

    /// The token that closes the delimiter at `position`, if any.
    fn closer_of(&mut self, position: usize) -> Option<usize> {
        let closers = self
            .closers
            .get_or_insert_with(|| matching_delimiters(&self.tokens));
        closers[position]
    }

    /// L0121 for an attribute where no item begins: in a type, an
    /// expression, a pattern, or a statement. Reported once and skipped,
    /// brackets and all.
    #[inline(never)]
    fn misplaced_attribute(&mut self) {
        let start = self.bump();
        self.eat(K::Bang);
        let closer = self.closer_of(self.position);
        let end = closer.map_or(self.current(), |closer| self.tokens[closer]);
        self.diagnostics.push(
            Diagnostic::error(
                "L0121",
                "an attribute goes before an item, and nothing else takes one",
                start.span.through(end.span),
            )
            .note("proof types use `@claim` or `@(condition)`, and `_` asks the elaborator for evidence"),
        );
        if let Some(closer) = closer {
            self.position = closer + 1;
        }
    }

    /// One attribute, `#[...]` or `#![...]`, read against the closed set,
    /// with whether it is inner. A malformed or unknown one is reported and
    /// skipped to its `]`, and is `None`; when the `[` is never closed, the
    /// rest of the file is skipped and this is `Err`.
    #[inline(never)]
    fn attribute(&mut self) -> ParseResult<Option<(Attribute, bool)>> {
        let start = self.bump();
        let inner = self.eat(K::Bang).is_some();
        let opening = self.bump();
        let closer = self.closer_of(self.position - 1);
        if let Ok(kind) = self.attribute_kind(inner) {
            if let Some(end) = self.eat(K::RBracket) {
                let attribute = Attribute {
                    kind,
                    span: start.span.through(end.span),
                };
                return Ok(Some((attribute, inner)));
            }
            let _ = self.close(K::RBracket, opening);
        }
        match closer {
            Some(closer) => {
                self.position = closer + 1;
                Ok(None)
            }
            None => {
                self.position = self.tokens.len() - 1;
                Err(())
            }
        }
    }

    /// The name and arguments between the brackets of an attribute.
    #[inline(never)]
    fn attribute_kind(&mut self, inner: bool) -> ParseResult<AttributeKind> {
        let name = self.expect(K::Name)?;
        let spelling = self.source.slice(name.span).unwrap_or_default().to_owned();
        match spelling.as_str() {
            "terminates" => {
                if !self.at(K::LParen) {
                    return Ok(AttributeKind::Terminates { decreases: None });
                }
                if inner {
                    return self.attribute_shape(
                        name.span,
                        "`decreases` names the measure of one function; at the top of the file write `#![terminates]`",
                    );
                }
                let decreases = self.decreases()?;
                Ok(AttributeKind::Terminates {
                    decreases: Some(decreases),
                })
            }
            "no_panic" | "no_alloc" | "no_io" => {
                if self.at(K::LParen) {
                    return self.attribute_shape(
                        name.span,
                        &format!("`#[{spelling}]` takes no arguments"),
                    );
                }
                Ok(match spelling.as_str() {
                    "no_panic" => AttributeKind::NoPanic,
                    "no_alloc" => AttributeKind::NoAlloc,
                    _ => AttributeKind::NoIo,
                })
            }
            "derive" => {
                if inner {
                    return self.attribute_shape(
                        name.span,
                        "`derive` goes on a struct or an enum, not at the top of the file",
                    );
                }
                if !self.at(K::LParen) {
                    return self.attribute_shape(
                        name.span,
                        "`#[derive]` takes a list of traits in parentheses, as in `#[derive(Clone, Copy)]`",
                    );
                }
                Ok(AttributeKind::Derive(self.derive_list()?))
            }
            _ => self.unknown_attribute(name.span, &spelling),
        }
    }

    /// `(decreases = expression)`
    #[inline(never)]
    fn decreases(&mut self) -> ParseResult<Expr> {
        let opening = self.expect(K::LParen)?;
        if !self.at_word("decreases") || self.peek(1) != K::Equal {
            return self.attribute_shape(
                self.current().span,
                "`#[terminates]` takes `decreases = expression` and nothing else",
            );
        }
        self.bump();
        self.bump();
        let expr = self.unrestricted(Self::expression)?;
        self.close(K::RParen, opening)?;
        Ok(expr)
    }

    /// `(Trait, a::Trait, ...)`
    #[inline(never)]
    fn derive_list(&mut self) -> ParseResult<Vec<Path>> {
        let opening = self.expect(K::LParen)?;
        let mut traits = Vec::new();
        while !self.at(K::RParen) && !self.at(K::Eof) {
            self.step();
            if !self.at_named() {
                return self.attribute_shape(
                    self.current().span,
                    "`#[derive]` lists traits by name, as in `#[derive(Clone, Copy)]`",
                );
            }
            traits.push(self.path()?);
            if self.eat(K::Comma).is_none() {
                break;
            }
        }
        self.close(K::RParen, opening)?;
        Ok(traits)
    }

    /// L0121: an attribute of the closed set in a shape it does not have.
    #[inline(never)]
    fn attribute_shape<T>(&mut self, span: Span, message: &str) -> ParseResult<T> {
        self.diagnostics
            .push(Diagnostic::error("L0121", message, span));
        Err(())
    }

    /// L0120: a name outside the closed set of attributes.
    #[inline(never)]
    fn unknown_attribute<T>(&mut self, span: Span, spelling: &str) -> ParseResult<T> {
        self.diagnostics.push(
            Diagnostic::error(
                "L0120",
                format!("`{spelling}` is not an attribute of Locus"),
                span,
            )
            .note(
                "the attributes are `#[terminates]`, `#[terminates(decreases = e)]`, `#[no_panic]`, `#[no_alloc]`, `#[no_io]`, and `#[derive(...)]`; Locus has no user-defined attributes",
            ),
        );
        Err(())
    }

    /// An attribute or a doc comment, or a keyword that begins an item in
    /// Rust and not yet in Locus.
    fn rust_item_start(&self) -> bool {
        if self.item_prefix_start() {
            return true;
        }
        self.at(K::Keyword)
            && matches!(
                self.source.slice(self.current().span),
                Some("async" | "extern" | "mod" | "static" | "trait" | "type" | "unsafe" | "use")
            )
    }

    /// Skips to where the next declaration begins. An item of Rust is a place
    /// to stop as well, past the one that failed, so that each is reported;
    /// inside an `impl` block, so is the `}` that closes it.
    fn recover_declaration(&mut self) {
        let start = self.position;
        let mut depth = 0usize;
        while !self.at(K::Eof) {
            self.step();
            let kind = self.current().kind;
            if depth == 0
                && (self.declaration_start()
                    || (self.position > start && self.rust_item_start())
                    || (self.in_impl && kind == K::RBrace))
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

/// Whether an expression can be assigned to: a name, or a field path
/// through names and positions, as `lock.failures` or `pair.0.x`.
fn is_place(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Name(_) => true,
        // `*self`, whole: the elaborator decides whether it can be written.
        ExprKind::Unary {
            operator: UnaryOp::Deref,
            expr: inner,
            ..
        } => matches!(inner.kind, ExprKind::Name(_)),
        ExprKind::Member { value, .. }
        | ExprKind::Index { value, .. }
        | ExprKind::Subscript { value, .. } => is_place(value),
        _ => false,
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
        format!(
            "`{keyword}` is a {} keyword and cannot be used as a name",
            if matches!(keyword, "prop" | "logic") {
                "Locus"
            } else {
                "Rust"
            }
        ),
        span,
    )
    .note(if matches!(keyword, "prop" | "logic") {
        "Locus reserves `prop` and `logic` in addition to every keyword of Rust"
    } else {
        "Locus reserves every keyword of Rust, whether or not it uses it yet"
    })
    .suggest(Suggestion {
        message: format!("rename it, for example to `{keyword}_`"),
        span,
        replacement: format!("{keyword}_"),
        applicability: Applicability::MaybeIncorrect,
    })
}

/// What a keyword that Locus does not use yet begins or marks in Rust.
/// `None` for the keywords Rust reserves without a use of its own.
fn keyword_construct(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "async" => "`async` is not in Locus yet",
        "await" => "`await` is not in Locus yet",
        "dyn" => "`dyn` trait objects are not in Locus yet",
        "extern" => "`extern` is not in Locus yet",
        "impl" => "`impl Trait` types are not in Locus yet",
        "mod" => "modules (`mod`) are not in Locus yet",
        "move" => "closures (`move`) are not in Locus yet",
        "ref" => "`ref` bindings are not in Locus yet",
        "static" => "`static` items are not in Locus yet",
        "trait" => "traits are not in Locus yet",
        "type" => "type aliases (`type`) are not in Locus yet",
        "unsafe" => "`unsafe` is not in Locus yet",
        "use" => "`use` declarations are not in Locus yet",
        "where" => "`where` clauses are not in Locus yet",
        _ => return None,
    })
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
        // Before an operand: a closure or a dereference.
        K::Or => "closures and or-patterns (`|`) are not in Locus yet".into(),
        K::Star => "dereferences and raw pointers (`*`) are not in Locus yet".into(),
        K::Question => "the `?` operator is not in Locus yet".into(),
        K::Dollar => "`$` belongs to macros, which are not in Locus yet".into(),
        K::DotDotEqual => {
            "inclusive ranges (`..=`) are not in Locus yet, except in the header of a `for`".into()
        }
        K::DotDotDot => "`...` is not in Locus yet".into(),
        K::Tilde | K::LeftArrow => {
            format!("`{spelling}` is a token of Rust with no meaning in Locus")
        }
        _ => return None,
    })
}

/// The binary operators with their binding powers. Every operator of Rust's
/// table that Locus lexes is here, whether or not the elaborator gives it a
/// meaning yet: what parses in Rust groups the same way here.
fn binary(kind: K) -> Option<(BinaryOp, (u8, u8))> {
    Some(match kind {
        K::Implies => (BinaryOp::Implies, IMPLIES),
        K::OrOr => (BinaryOp::Or, OR),
        K::AndAnd => (BinaryOp::And, AND),
        K::EqualEqual => (BinaryOp::Equal, COMPARISON),
        K::BangEqual => (BinaryOp::NotEqual, COMPARISON),
        K::Less => (BinaryOp::Less, COMPARISON),
        K::LessEqual => (BinaryOp::LessEqual, COMPARISON),
        K::Greater => (BinaryOp::Greater, COMPARISON),
        K::GreaterEqual => (BinaryOp::GreaterEqual, COMPARISON),
        K::Or => (BinaryOp::BitOr, BIT_OR),
        K::Caret => (BinaryOp::BitXor, BIT_XOR),
        K::And => (BinaryOp::BitAnd, BIT_AND),
        K::ShiftLeft => (BinaryOp::Shl, SHIFT),
        K::ShiftRight => (BinaryOp::Shr, SHIFT),
        K::Plus => (BinaryOp::Add, SUM),
        K::Minus => (BinaryOp::Sub, SUM),
        K::Star => (BinaryOp::Mul, PRODUCT),
        K::Slash => (BinaryOp::Div, PRODUCT),
        K::Percent => (BinaryOp::Rem, PRODUCT),
        _ => return None,
    })
}

/// A migration diagnostic for a legacy proposition arm. The parser retains
/// legacy syntax while the named-proposition preview is being reconciled;
/// the elaborator calls this when that preview is active. Only replacements
/// whose body follows mechanically from the old syntax are offered as fixes.
pub fn legacy_prop_migration(arm: &PropVariant, source: &SourceFile) -> Diagnostic {
    let diagnostic = Diagnostic::error(
        "L0152",
        "a proposition arm states its body with `=> { ... }`",
        arm.span,
    )
    .note("declare witnesses inside the arm and supply evidence after `@` when constructing it");
    let text = |span| source.slice(span).unwrap_or_default();
    let replacement = if arm.fields.is_empty() {
        match &arm.target {
            Some(target) if !matches!(target.kind, ExprKind::Call { .. }) => Some(format!(
                "{} => {{ prop!({}) }}",
                arm.name.text,
                text(target.span)
            )),
            None => Some(format!("{} => {{ prop!(true) }}", arm.name.text)),
            _ => None,
        }
    } else if arm.target.is_none() {
        // Proof fields become the one computed arm body; other fields remain
        // witnesses. A body depending on a proof binding needs a manual edit.
        let mut witnesses = Vec::new();
        let mut claims = Vec::new();
        for (index, field) in arm.fields.iter().enumerate() {
            match &field.ty.kind {
                TypeKind::Proof(claim) => claims.push(format!("({})", text(claim.span))),
                _ => witnesses.push(match &field.name {
                    Some(_) => text(field.span).to_owned(),
                    None => format!("witness_{index}: {}", text(field.ty.span)),
                }),
            }
        }
        let witness_text = match arm.shape {
            VariantShape::Unit => String::new(),
            VariantShape::Tuple => format!("({})", witnesses.join(", ")),
            VariantShape::Struct => format!(" {{ {} }}", witnesses.join(", ")),
        };
        let body = if claims.is_empty() {
            "true".to_owned()
        } else {
            claims.join(" && ")
        };
        Some(format!(
            "{}{witness_text} => {{ prop!({body}) }}",
            arm.name.text
        ))
    } else {
        None
    };
    match replacement {
        Some(replacement) => diagnostic.suggest(Suggestion {
            message: "write a computed proposition body".into(),
            span: arm.span,
            replacement,
            applicability: Applicability::MaybeIncorrect,
        }),
        None => diagnostic.note("an explicitly indexed conclusion needs a manual rewrite into witness conditions in the arm body"),
    }
}
