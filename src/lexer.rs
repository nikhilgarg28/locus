//! Handwritten tokenizer. Literal values are interpreted only during elaboration.

use crate::diagnostic::Diagnostic;
use crate::source::{SourceFile, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Name,
    Integer,
    Underscore,
    Fn,
    Def,
    Const,
    Let,
    If,
    Else,
    Forall,
    Exists,
    Struct,
    Enum,
    Match,
    Loop,
    For,
    In,
    Break,
    Continue,
    True,
    False,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Semicolon,
    Dot,
    DotDot,
    PathSep,
    Hash,
    At,
    Plus,
    Bang,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Arrow,
    Implies,
    Error,
    Eof,
}

impl TokenKind {
    pub fn description(self) -> &'static str {
        match self {
            Self::Name => "an identifier",
            Self::Integer => "an integer",
            Self::Underscore => "`_`",
            Self::Fn => "`fn`",
            Self::Def => "`def`",
            Self::Const => "`const`",
            Self::Let => "`let`",
            Self::If => "`if`",
            Self::Else => "`else`",
            Self::Forall => "`forall`",
            Self::Exists => "`exists`",
            Self::Struct => "`struct`",
            Self::Enum => "`enum`",
            Self::Match => "`match`",
            Self::Loop => "`loop`",
            Self::For => "`for`",
            Self::In => "`in`",
            Self::Break => "`break`",
            Self::Continue => "`continue`",
            Self::True => "`true`",
            Self::False => "`false`",
            Self::LParen => "`(`",
            Self::RParen => "`)`",
            Self::LBrace => "`{`",
            Self::RBrace => "`}`",
            Self::LBracket => "`[`",
            Self::RBracket => "`]`",
            Self::Comma => "`,`",
            Self::Colon => "`:`",
            Self::Semicolon => "`;`",
            Self::Dot => "`.`",
            Self::DotDot => "`..`",
            Self::PathSep => "`::`",
            Self::Hash => "`#`",
            Self::At => "`@`",
            Self::Plus => "`+`",
            Self::Bang => "`!`",
            Self::Equal => "`=`",
            Self::EqualEqual => "`==`",
            Self::BangEqual => "`!=`",
            Self::Less => "`<`",
            Self::LessEqual => "`<=`",
            Self::Greater => "`>`",
            Self::GreaterEqual => "`>=`",
            Self::AndAnd => "`&&`",
            Self::OrOr => "`||`",
            Self::Arrow => "`->`",
            Self::Implies => "`=>`",
            Self::Error => "an invalid token",
            Self::Eof => "end of file",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(source: &SourceFile) -> Lexed {
    Lexer {
        source,
        position: 0,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    }
    .run()
}

struct Lexer<'a> {
    source: &'a SourceFile,
    position: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn run(mut self) -> Lexed {
        while let Some(character) = self.current() {
            let start = self.position;
            if character.is_whitespace() {
                self.advance();
            } else if self.remaining().starts_with("//") {
                while self.current().is_some_and(|character| character != '\n') {
                    self.advance();
                }
            } else if self.remaining().starts_with("/*") {
                self.comment();
            } else if character.is_ascii_digit() {
                self.number();
            } else if character.is_alphabetic() || character == '_' {
                self.name();
            } else if character == '"' {
                self.unsupported_string();
            } else {
                let two = [
                    ("==", TokenKind::EqualEqual),
                    ("!=", TokenKind::BangEqual),
                    ("<=", TokenKind::LessEqual),
                    (">=", TokenKind::GreaterEqual),
                    ("&&", TokenKind::AndAnd),
                    ("||", TokenKind::OrOr),
                    ("->", TokenKind::Arrow),
                    ("=>", TokenKind::Implies),
                    ("::", TokenKind::PathSep),
                    ("..", TokenKind::DotDot),
                ];
                if let Some((text, kind)) = two
                    .iter()
                    .find(|(text, _)| self.remaining().starts_with(text))
                {
                    self.position += text.len();
                    self.emit(*kind, start);
                    continue;
                }
                self.advance();
                let kind = match character {
                    '(' => TokenKind::LParen,
                    ')' => TokenKind::RParen,
                    '{' => TokenKind::LBrace,
                    '}' => TokenKind::RBrace,
                    '[' => TokenKind::LBracket,
                    ']' => TokenKind::RBracket,
                    ',' => TokenKind::Comma,
                    ':' => TokenKind::Colon,
                    ';' => TokenKind::Semicolon,
                    '.' => TokenKind::Dot,
                    '#' => TokenKind::Hash,
                    '@' => TokenKind::At,
                    '+' => TokenKind::Plus,
                    '!' => TokenKind::Bang,
                    '=' => TokenKind::Equal,
                    '<' => TokenKind::Less,
                    '>' => TokenKind::Greater,
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            "L0001",
                            format!("unexpected character {character:?}"),
                            self.span(start),
                        ));
                        TokenKind::Error
                    }
                };
                self.emit(kind, start);
            }
        }
        self.emit(TokenKind::Eof, self.position);
        Lexed {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn remaining(&self) -> &str {
        &self.source.text()[self.position..]
    }

    fn current(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.current() {
            self.position += character.len_utf8();
        }
    }

    fn span(&self, start: usize) -> Span {
        Span::new(self.source.id, start, self.position)
    }

    fn emit(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start),
        });
    }

    fn comment(&mut self) {
        let start = self.position;
        self.position += 2;
        let mut depth = 1usize;
        while self.current().is_some() {
            if self.remaining().starts_with("/*") {
                depth += 1;
                self.position += 2;
            } else if self.remaining().starts_with("*/") {
                depth -= 1;
                self.position += 2;
                if depth == 0 {
                    return;
                }
            } else {
                self.advance();
            }
        }
        self.diagnostics.push(
            Diagnostic::error(
                "L0002",
                "unterminated block comment",
                Span::new(self.source.id, start, start + 2),
            )
            .note("close this comment with `*/`; nested block comments must each be closed"),
        );
        self.emit(TokenKind::Error, start);
    }

    fn number(&mut self) {
        let start = self.position;
        while self
            .current()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        {
            self.advance();
        }
        let spelling = &self.source.text()[start..self.position];
        let valid = spelling
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'_')
            && !spelling.ends_with('_')
            && !spelling.contains("__");
        if valid {
            self.emit(TokenKind::Integer, start);
        } else {
            self.diagnostics.push(Diagnostic::error(
                "L0003", "invalid decimal integer literal", self.span(start),
            ).note("use decimal digits with optional single underscores between digits; bases and type suffixes are not supported yet"));
            self.emit(TokenKind::Error, start);
        }
    }

    fn name(&mut self) {
        let start = self.position;
        while self
            .current()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        {
            self.advance();
        }
        let spelling = &self.source.text()[start..self.position];
        if !spelling.is_ascii() {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0004",
                    "non-ASCII identifiers are not supported in the initial core",
                    self.span(start),
                )
                .note("use ASCII letters, digits, and underscores for identifiers"),
            );
            self.emit(TokenKind::Error, start);
            return;
        }
        if spelling == "r" && self.remaining().starts_with('#') {
            self.advance();
            while self
                .current()
                .is_some_and(|character| character.is_alphanumeric() || character == '_')
            {
                self.advance();
            }
            self.diagnostics.push(Diagnostic::error(
                "L0005",
                "raw identifiers and raw strings are not supported in the initial core",
                self.span(start),
            ));
            self.emit(TokenKind::Error, start);
            return;
        }
        let kind = match spelling {
            "fn" => TokenKind::Fn,
            "def" => TokenKind::Def,
            "const" => TokenKind::Const,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "forall" => TokenKind::Forall,
            "exists" => TokenKind::Exists,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "loop" => TokenKind::Loop,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "_" => TokenKind::Underscore,
            _ => TokenKind::Name,
        };
        self.emit(kind, start);
    }

    fn unsupported_string(&mut self) {
        let start = self.position;
        self.advance();
        while let Some(character) = self.current() {
            if character == '\n' || character == '\r' {
                break;
            }
            self.advance();
            if character == '"' {
                break;
            }
            if character == '\\'
                && self
                    .current()
                    .is_some_and(|next| next != '\n' && next != '\r')
            {
                self.advance();
            }
        }
        self.diagnostics.push(Diagnostic::error(
            "L0006",
            "string literals are not supported in the initial core",
            self.span(start),
        ));
        self.emit(TokenKind::Error, start);
    }
}
