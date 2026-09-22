//! Handwritten tokenizer. Locus source tokenizes as Rust: every Rust keyword
//! is reserved and every Rust token is recognized, and there is no token Rust
//! lacks. The words of Locus alone (`math`, `prop`, `forall`, `exists`, the
//! retired `def`) are names, which the parser reads in context. A literal
//! form Locus does not have yet is reported here and becomes an error token;
//! an operator or a keyword it does not use yet is a token, and the parser
//! reports it where it stands. Doc comments are tokens too, with their text,
//! so that the parser can keep them on the items they document. Literals are
//! decoded here; whether a value fits a type is decided during elaboration.

use crate::ast::{IntegerLiteral, IntegerSuffix};
use crate::diagnostic::Diagnostic;
use crate::kernel::Natural;
use crate::source::{SourceFile, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Name,
    Integer,
    String,
    /// `/// text` or `/** text */`, which documents the item after it. The
    /// text is in `Lexed::literals`, as a string.
    OuterDoc,
    /// `//! text` or `/*! text */`, which documents the file.
    InnerDoc,
    Underscore,
    Fn,
    Const,
    Let,
    If,
    Else,
    Struct,
    Enum,
    Match,
    Loop,
    For,
    In,
    Break,
    Continue,
    While,
    Return,
    Mut,
    True,
    False,
    As,
    /// A Rust keyword that Locus reserves and does not use yet.
    Keyword,
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
    DotDotDot,
    DotDotEqual,
    PathSep,
    Hash,
    At,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    And,
    Or,
    ShiftLeft,
    ShiftRight,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    CaretEqual,
    AndEqual,
    OrEqual,
    ShiftLeftEqual,
    ShiftRightEqual,
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
    LeftArrow,
    Implies,
    Question,
    Dollar,
    Tilde,
    Error,
    Eof,
}

impl TokenKind {
    pub fn description(self) -> &'static str {
        match self {
            Self::Name => "an identifier",
            Self::Integer => "an integer",
            Self::String => "a string",
            Self::OuterDoc => "a doc comment",
            Self::InnerDoc => "an inner doc comment (`//!`)",
            Self::Underscore => "`_`",
            Self::Fn => "`fn`",
            Self::Const => "`const`",
            Self::Let => "`let`",
            Self::If => "`if`",
            Self::Else => "`else`",
            Self::Struct => "`struct`",
            Self::Enum => "`enum`",
            Self::Match => "`match`",
            Self::Loop => "`loop`",
            Self::For => "`for`",
            Self::In => "`in`",
            Self::Break => "`break`",
            Self::Continue => "`continue`",
            Self::While => "`while`",
            Self::Return => "`return`",
            Self::Mut => "`mut`",
            Self::True => "`true`",
            Self::False => "`false`",
            Self::As => "`as`",
            Self::Keyword => "a Rust keyword",
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
            Self::DotDotDot => "`...`",
            Self::DotDotEqual => "`..=`",
            Self::PathSep => "`::`",
            Self::Hash => "`#`",
            Self::At => "`@`",
            Self::Plus => "`+`",
            Self::Minus => "`-`",
            Self::Star => "`*`",
            Self::Slash => "`/`",
            Self::Percent => "`%`",
            Self::Caret => "`^`",
            Self::And => "`&`",
            Self::Or => "`|`",
            Self::ShiftLeft => "`<<`",
            Self::ShiftRight => "`>>`",
            Self::PlusEqual => "`+=`",
            Self::MinusEqual => "`-=`",
            Self::StarEqual => "`*=`",
            Self::SlashEqual => "`/=`",
            Self::PercentEqual => "`%=`",
            Self::CaretEqual => "`^=`",
            Self::AndEqual => "`&=`",
            Self::OrEqual => "`|=`",
            Self::ShiftLeftEqual => "`<<=`",
            Self::ShiftRightEqual => "`>>=`",
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
            Self::LeftArrow => "`<-`",
            Self::Implies => "`=>`",
            Self::Question => "`?`",
            Self::Dollar => "`$`",
            Self::Tilde => "`~`",
            Self::Error => "an invalid token",
            Self::Eof => "end of file",
        }
    }

    /// A strict or reserved keyword of Rust, which no name may be spelled as.
    /// Every other word is a name, the words of Locus alone included.
    pub fn is_rust_keyword(self) -> bool {
        matches!(
            self,
            Self::Fn
                | Self::Const
                | Self::Let
                | Self::If
                | Self::Else
                | Self::Struct
                | Self::Enum
                | Self::Match
                | Self::Loop
                | Self::For
                | Self::In
                | Self::Break
                | Self::Continue
                | Self::While
                | Self::Return
                | Self::Mut
                | Self::True
                | Self::False
                | Self::As
                | Self::Keyword
        )
    }
}

/// Punctuation, longest spelling first, so that the first match is the token.
const PUNCTUATION: &[(&str, TokenKind)] = &[
    ("<<=", TokenKind::ShiftLeftEqual),
    (">>=", TokenKind::ShiftRightEqual),
    ("...", TokenKind::DotDotDot),
    ("..=", TokenKind::DotDotEqual),
    ("==", TokenKind::EqualEqual),
    ("!=", TokenKind::BangEqual),
    ("<=", TokenKind::LessEqual),
    (">=", TokenKind::GreaterEqual),
    ("&&", TokenKind::AndAnd),
    ("||", TokenKind::OrOr),
    ("->", TokenKind::Arrow),
    ("<-", TokenKind::LeftArrow),
    ("=>", TokenKind::Implies),
    ("::", TokenKind::PathSep),
    ("..", TokenKind::DotDot),
    ("<<", TokenKind::ShiftLeft),
    (">>", TokenKind::ShiftRight),
    ("+=", TokenKind::PlusEqual),
    ("-=", TokenKind::MinusEqual),
    ("*=", TokenKind::StarEqual),
    ("/=", TokenKind::SlashEqual),
    ("%=", TokenKind::PercentEqual),
    ("^=", TokenKind::CaretEqual),
    ("&=", TokenKind::AndEqual),
    ("|=", TokenKind::OrEqual),
    ("(", TokenKind::LParen),
    (")", TokenKind::RParen),
    ("{", TokenKind::LBrace),
    ("}", TokenKind::RBrace),
    ("[", TokenKind::LBracket),
    ("]", TokenKind::RBracket),
    (",", TokenKind::Comma),
    (":", TokenKind::Colon),
    (";", TokenKind::Semicolon),
    (".", TokenKind::Dot),
    ("#", TokenKind::Hash),
    ("@", TokenKind::At),
    ("+", TokenKind::Plus),
    ("-", TokenKind::Minus),
    ("*", TokenKind::Star),
    ("/", TokenKind::Slash),
    ("%", TokenKind::Percent),
    ("^", TokenKind::Caret),
    ("&", TokenKind::And),
    ("|", TokenKind::Or),
    ("!", TokenKind::Bang),
    ("=", TokenKind::Equal),
    ("<", TokenKind::Less),
    (">", TokenKind::Greater),
    ("?", TokenKind::Question),
    ("$", TokenKind::Dollar),
    ("~", TokenKind::Tilde),
];

/// The value of an `Integer` or a `String` token, or the text of a doc
/// comment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Literal {
    Integer(IntegerLiteral),
    String(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Where `Lexed::literals` holds the value of an `Integer` or a `String`
    /// token, and zero for any other. It is this narrow so that a token stays
    /// four words: the parser keeps tokens in the frames of its recursion.
    pub literal: u32,
}

#[derive(Debug)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub literals: Vec<Literal>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Lexed {
    pub fn literal(&self, token: Token) -> Option<&Literal> {
        matches!(
            token.kind,
            TokenKind::Integer | TokenKind::String | TokenKind::OuterDoc | TokenKind::InnerDoc
        )
        .then(|| &self.literals[token.literal as usize])
    }
}

pub fn lex(source: &SourceFile) -> Lexed {
    Lexer {
        source,
        position: 0,
        tokens: Vec::new(),
        literals: Vec::new(),
        diagnostics: Vec::new(),
    }
    .run()
}

struct Lexer<'a> {
    source: &'a SourceFile,
    position: usize,
    tokens: Vec<Token>,
    literals: Vec<Literal>,
    diagnostics: Vec<Diagnostic>,
}

/// What a backslash in a string literal stands for.
enum Escape {
    Character(char),
    /// A backslash at the end of a line, which continues the string after the
    /// next line's indentation.
    Nothing,
    Invalid,
}

fn continues_name(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

impl<'a> Lexer<'a> {
    fn run(mut self) -> Lexed {
        while let Some(character) = self.current() {
            let start = self.position;
            if character.is_whitespace() {
                self.advance();
            } else if self.remaining().starts_with("//") {
                self.line_comment();
            } else if self.remaining().starts_with("/*") {
                self.comment();
            } else if character.is_ascii_digit() {
                self.number();
            } else if character.is_alphabetic() || character == '_' {
                self.name();
            } else if character == '"' {
                self.string();
            } else if character == '\'' {
                self.quote();
            } else if let Some((text, kind)) = PUNCTUATION
                .iter()
                .find(|(text, _)| self.remaining().starts_with(text))
            {
                self.position += text.len();
                self.emit(*kind, start);
            } else {
                self.advance();
                self.diagnostics.push(Diagnostic::error(
                    "L0001",
                    format!("unexpected character {character:?}"),
                    self.span(start),
                ));
                self.emit(TokenKind::Error, start);
            }
        }
        self.emit(TokenKind::Eof, self.position);
        Lexed {
            tokens: self.tokens,
            literals: self.literals,
            diagnostics: self.diagnostics,
        }
    }

    fn remaining(&self) -> &'a str {
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

    fn advance_while(&mut self, accept: impl Fn(char) -> bool) {
        while self.current().is_some_and(&accept) {
            self.advance();
        }
    }

    fn span(&self, start: usize) -> Span {
        Span::new(self.source.id, start, self.position)
    }

    /// The quote the lexer stands on, which is where an unterminated literal
    /// is reported.
    fn quote_span(&self) -> Span {
        Span::new(self.source.id, self.position, self.position + 1)
    }

    fn emit(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start),
            literal: 0,
        });
    }

    fn emit_literal(&mut self, kind: TokenKind, start: usize, literal: Literal) {
        self.tokens.push(Token {
            kind,
            span: self.span(start),
            literal: u32::try_from(self.literals.len()).expect("fewer than 2^32 literals"),
        });
        self.literals.push(literal);
    }

    fn invalid(&mut self, diagnostic: Diagnostic, start: usize) {
        self.diagnostics.push(diagnostic);
        self.emit(TokenKind::Error, start);
    }

    /// A token of Rust that is lexed whole and has no meaning in Locus yet.
    fn not_yet(&mut self, what: &str, start: usize) {
        let diagnostic = Diagnostic::error(
            "L0005",
            format!("{what} are not in Locus yet"),
            self.span(start),
        );
        self.invalid(diagnostic, start);
    }

    fn unterminated(&mut self, what: &str, quote: Span, start: usize) {
        let diagnostic = Diagnostic::error("L0006", format!("unterminated {what} literal"), quote)
            .note("the literal runs to the end of the file; close it with a matching quote");
        self.invalid(diagnostic, start);
    }

    /// `// ...` to the end of the line. As in Rust, `///` (but not `////`)
    /// is a doc comment and `//!` an inner one, and each is a token.
    fn line_comment(&mut self) {
        let start = self.position;
        let rest = self.remaining();
        let doc = if rest.starts_with("//!") {
            Some(TokenKind::InnerDoc)
        } else if rest.starts_with("///") && !rest.starts_with("////") {
            Some(TokenKind::OuterDoc)
        } else {
            None
        };
        self.position += if doc.is_some() { 3 } else { 2 };
        let text_start = self.position;
        while self.current().is_some_and(|character| character != '\n') {
            self.advance();
        }
        if let Some(kind) = doc {
            let text = self.source.text()[text_start..self.position]
                .trim_end_matches('\r')
                .to_owned();
            self.emit_literal(kind, start, Literal::String(text));
        }
    }

    /// `/* ... */`, nesting. As in Rust, `/**` (but not `/***` or `/**/`) is
    /// a doc comment and `/*!` an inner one.
    fn comment(&mut self) {
        let start = self.position;
        let rest = self.remaining();
        let doc = if rest.starts_with("/*!") {
            Some(TokenKind::InnerDoc)
        } else if rest.starts_with("/**") && !rest.starts_with("/***") && !rest.starts_with("/**/")
        {
            Some(TokenKind::OuterDoc)
        } else {
            None
        };
        self.position += if doc.is_some() { 3 } else { 2 };
        let text_start = self.position;
        let mut depth = 1usize;
        while self.current().is_some() {
            if self.remaining().starts_with("/*") {
                depth += 1;
                self.position += 2;
            } else if self.remaining().starts_with("*/") {
                depth -= 1;
                self.position += 2;
                if depth == 0 {
                    if let Some(kind) = doc {
                        let text = self.source.text()[text_start..self.position - 2].to_owned();
                        self.emit_literal(kind, start, Literal::String(text));
                    }
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

    /// An integer in any of Rust's four bases, or a float, which is lexed as
    /// Rust lexes it and reported.
    fn number(&mut self) {
        let start = self.position;
        // `pair.0.1` is two projections, not a projection by the float `0.1`.
        let projection = self
            .tokens
            .last()
            .is_some_and(|token| token.kind == TokenKind::Dot);
        let radix = match self.remaining().as_bytes() {
            [b'0', b'x', ..] => 16,
            [b'0', b'o', ..] => 8,
            [b'0', b'b', ..] => 2,
            _ => 10,
        };
        if radix != 10 {
            self.position += 2;
        }
        // As in rustc, a binary or octal literal takes every decimal digit,
        // so that `0b12` is one literal with a bad digit and not two tokens.
        let digits_start = self.position;
        self.advance_while(|character| {
            character == '_'
                || if radix == 16 {
                    character.is_ascii_hexdigit()
                } else {
                    character.is_ascii_digit()
                }
        });
        let digits_end = self.position;
        let mut float = false;
        if radix == 10 && !projection {
            float |= self.fraction();
            float |= self.exponent();
        }
        let suffix_start = self.position;
        self.advance_while(continues_name);
        let text = self.source.text();
        let digits = &text[digits_start..digits_end];
        let suffix = &text[suffix_start..self.position];
        let suffix_span = Span::new(self.source.id, suffix_start, self.position);

        if float || (radix == 10 && matches!(suffix, "f32" | "f64")) {
            if !matches!(suffix, "" | "f32" | "f64") {
                let diagnostic = Diagnostic::error(
                    "L0003",
                    format!("invalid suffix `{suffix}` for a float literal"),
                    suffix_span,
                );
                return self.invalid(diagnostic, start);
            }
            return self.not_yet("float literals", start);
        }
        if !digits.bytes().any(|byte| byte != b'_') {
            let diagnostic = Diagnostic::error(
                "L0003",
                format!("no digits after `{}`", &text[start..digits_start]),
                self.span(start),
            );
            return self.invalid(diagnostic, start);
        }
        if let Some((offset, digit)) = digits
            .char_indices()
            .find(|(_, digit)| *digit != '_' && !digit.is_digit(radix))
        {
            let at = digits_start + offset;
            let diagnostic = Diagnostic::error(
                "L0003",
                format!("`{digit}` is not a digit of a base {radix} literal"),
                Span::new(self.source.id, at, at + 1),
            );
            return self.invalid(diagnostic, start);
        }
        let suffix = match suffix {
            "" => None,
            _ => match IntegerSuffix::from_name(suffix) {
                Some(suffix) => Some(suffix),
                None => {
                    let diagnostic = Diagnostic::error(
                        "L0003",
                        format!("invalid suffix `{suffix}` for an integer literal"),
                        suffix_span,
                    )
                    .note("the suffixes are `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `i8`, `i16`, `i32`, `i64`, `i128`, and `isize`");
                    return self.invalid(diagnostic, start);
                }
            },
        };
        let value = natural(digits, radix);
        self.emit_literal(
            TokenKind::Integer,
            start,
            Literal::Integer(IntegerLiteral { value, suffix }),
        );
    }

    /// The `.5` of `1.5`, and the `.` of `1.`. In `0..n` the dots are a range,
    /// and in `1.max(2)` the dot begins a method call.
    fn fraction(&mut self) -> bool {
        let mut rest = self.remaining().chars();
        if rest.next() != Some('.') {
            return false;
        }
        if rest
            .next()
            .is_some_and(|next| next == '.' || next == '_' || next.is_alphabetic())
        {
            return false;
        }
        self.advance();
        self.advance_while(|character| character.is_ascii_digit() || character == '_');
        true
    }

    /// The `e5` or `E-5` of a float. An `e` with no digits after it begins a
    /// suffix, which is then reported as an invalid one.
    fn exponent(&mut self) -> bool {
        let mut rest = self.remaining().chars();
        if !matches!(rest.next(), Some('e' | 'E')) {
            return false;
        }
        let mut next = rest.next();
        let signed = matches!(next, Some('+' | '-'));
        if signed {
            next = rest.next();
        }
        if !next.is_some_and(|digit| digit.is_ascii_digit()) {
            return false;
        }
        self.position += 1 + usize::from(signed);
        self.advance_while(|character| character.is_ascii_digit() || character == '_');
        true
    }

    fn name(&mut self) {
        let start = self.position;
        self.advance_while(continues_name);
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
        if matches!(spelling, "r" | "br" | "cr")
            && self.remaining().trim_start_matches('#').starts_with('"')
        {
            return self.raw_string(spelling, start);
        }
        if spelling == "r" && self.remaining().starts_with('#') {
            self.advance();
            self.advance_while(continues_name);
            return self.not_yet("raw identifiers", start);
        }
        if matches!(spelling, "b" | "c") && self.remaining().starts_with('"') {
            let quote = self.quote_span();
            let what = if spelling == "b" {
                "byte string"
            } else {
                "C string"
            };
            self.advance();
            return if self.skip_quoted() {
                self.not_yet(&format!("{what}s"), start)
            } else {
                self.unterminated(what, quote, start)
            };
        }
        if spelling == "b" && self.remaining().starts_with('\'') {
            let quote = self.quote_span();
            self.advance();
            return if self.skip_character() {
                self.not_yet("byte literals", start)
            } else {
                self.unterminated("byte", quote, start)
            };
        }
        let kind = match spelling {
            "fn" => TokenKind::Fn,
            "const" => TokenKind::Const,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "match" => TokenKind::Match,
            "loop" => TokenKind::Loop,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "mut" => TokenKind::Mut,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "_" => TokenKind::Underscore,
            "as" => TokenKind::As,
            // The other strict keywords of Rust 2024, then the reserved ones.
            // The weak keywords (`union`, `macro_rules`, `raw`, `safe`) are
            // names, as they are in Rust.
            "async" | "await" | "crate" | "dyn" | "extern" | "impl" | "mod" | "move" | "pub"
            | "ref" | "self" | "Self" | "static" | "super" | "trait" | "type" | "unsafe"
            | "use" | "where" => TokenKind::Keyword,
            "abstract" | "become" | "box" | "do" | "final" | "gen" | "macro" | "override"
            | "priv" | "try" | "typeof" | "unsized" | "virtual" | "yield" => TokenKind::Keyword,
            _ => TokenKind::Name,
        };
        self.emit(kind, start);
    }

    /// `r"..."`, `r#"..."#`, and the same after `br` and `cr`: no escapes, and
    /// the closing quote carries as many `#` as the opening one.
    fn raw_string(&mut self, prefix: &str, start: usize) {
        let hashes = self.remaining().len() - self.remaining().trim_start_matches('#').len();
        self.position += hashes;
        let quote = self.quote_span();
        self.position += 1;
        let closing = format!("\"{}", "#".repeat(hashes));
        let (what, plural) = match prefix {
            "r" => ("raw string", "raw strings"),
            "br" => ("raw byte string", "raw byte strings"),
            _ => ("raw C string", "raw C strings"),
        };
        match self.remaining().find(&closing) {
            Some(offset) => {
                self.position += offset + closing.len();
                self.not_yet(plural, start);
            }
            None => {
                self.position = self.source.text().len();
                self.unterminated(what, quote, start);
            }
        }
    }

    /// A string literal with Rust's escapes. It may span lines, as in Rust.
    fn string(&mut self) {
        let start = self.position;
        self.advance();
        let quote = self.span(start);
        let mut value = String::new();
        let mut valid = true;
        loop {
            let at = self.position;
            let Some(character) = self.current() else {
                return self.unterminated("string", quote, start);
            };
            self.advance();
            match character {
                '"' => break,
                '\\' => match self.escape(at) {
                    Escape::Character(character) => value.push(character),
                    Escape::Nothing => {}
                    Escape::Invalid => valid = false,
                },
                // A line ending is one `\n` however the file spells it.
                '\r' if self.current() == Some('\n') => {}
                _ => value.push(character),
            }
        }
        if valid {
            self.emit_literal(TokenKind::String, start, Literal::String(value));
        } else {
            self.emit(TokenKind::Error, start);
        }
    }

    /// The escape whose backslash, at `at`, has just been taken. An invalid
    /// escape is reported here, with a span of its own.
    fn escape(&mut self, at: usize) -> Escape {
        // At the end of the file the string is unterminated, which is the
        // one thing to report.
        let Some(character) = self.current() else {
            return Escape::Invalid;
        };
        self.advance();
        let message = match character {
            'n' => return Escape::Character('\n'),
            'r' => return Escape::Character('\r'),
            't' => return Escape::Character('\t'),
            '\\' => return Escape::Character('\\'),
            '0' => return Escape::Character('\0'),
            '\'' => return Escape::Character('\''),
            '"' => return Escape::Character('"'),
            '\n' | '\r' => {
                self.advance_while(|character| matches!(character, ' ' | '\t' | '\n' | '\r'));
                return Escape::Nothing;
            }
            'x' => {
                let digits_start = self.position;
                for _ in 0..2 {
                    if self
                        .current()
                        .is_some_and(|digit| digit.is_ascii_hexdigit())
                    {
                        self.advance();
                    }
                }
                let digits = &self.source.text()[digits_start..self.position];
                match u8::from_str_radix(digits, 16) {
                    Ok(value) if digits.len() == 2 && value <= 0x7f => {
                        return Escape::Character(char::from(value));
                    }
                    Ok(_) if digits.len() == 2 => {
                        "a `\\x` escape in a string goes up to `\\x7F`".to_owned()
                    }
                    _ => "a `\\x` escape takes exactly two hexadecimal digits".to_owned(),
                }
            }
            'u' => match self.unicode_escape() {
                Ok(character) => return Escape::Character(character),
                Err(message) => message.to_owned(),
            },
            other => format!("unknown escape `\\{}`", other.escape_default()),
        };
        self.diagnostics.push(
            Diagnostic::error("L0007", message, self.span(at)).note(
                "the escapes are `\\n`, `\\r`, `\\t`, `\\\\`, `\\0`, `\\'`, `\\\"`, `\\x7F`, `\\u{10FFFF}`, and a backslash at the end of a line",
            ),
        );
        Escape::Invalid
    }

    /// The `{...}` of a `\u{...}` escape: one to six hexadecimal digits that
    /// name a Unicode scalar value. Underscores may separate the digits.
    fn unicode_escape(&mut self) -> Result<char, &'static str> {
        if self.current() != Some('{') {
            return Err("a `\\u` escape is written `\\u{...}`, with braces");
        }
        self.advance();
        let digits_start = self.position;
        self.advance_while(|character| character.is_ascii_hexdigit() || character == '_');
        let digits = self.source.text()[digits_start..self.position].replace('_', "");
        if self.current() != Some('}') {
            return Err(
                "a `\\u{...}` escape holds hexadecimal digits and ends with a closing brace",
            );
        }
        self.advance();
        if digits.is_empty() {
            return Err("a `\\u{...}` escape needs at least one hexadecimal digit");
        }
        if digits.len() > 6 {
            return Err("a `\\u{...}` escape takes at most six hexadecimal digits");
        }
        u32::from_str_radix(&digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or("a `\\u{...}` escape names a Unicode scalar value: at most `10FFFF`, and not a surrogate")
    }

    /// Skips the rest of a quoted literal that is only reported, after its
    /// opening quote. False when the file ends first.
    fn skip_quoted(&mut self) -> bool {
        while let Some(character) = self.current() {
            self.advance();
            match character {
                '"' => return true,
                '\\' => self.advance(),
                _ => {}
            }
        }
        false
    }

    /// `'a` is a lifetime or a label and `'a'` a character, told apart as
    /// rustc tells them: a name after the quote is a lifetime unless a quote
    /// closes it.
    fn quote(&mut self) {
        let start = self.position;
        let quote = Span::new(self.source.id, start, start + 1);
        self.advance();
        let mut rest = self.remaining().chars();
        let (first, second) = (rest.next(), rest.next());
        if second != Some('\'') && first.is_some_and(continues_name) {
            self.advance_while(continues_name);
            if self.current() != Some('\'') {
                return self.not_yet("lifetimes and loop labels", start);
            }
            // `'ab'`: a character literal with too much in it, and no lifetime.
            self.advance();
        } else if !self.skip_character() {
            return self.unterminated("character", quote, start);
        }
        self.not_yet("character literals", start);
    }

    /// Skips the rest of a character or byte literal, after its opening
    /// quote, stopping where rustc stops: at the closing quote, or before a
    /// line end or a comment when the literal is not closed.
    fn skip_character(&mut self) -> bool {
        let mut rest = self.remaining().chars();
        let (first, second) = (rest.next(), rest.next());
        if second == Some('\'') && first != Some('\\') {
            self.advance();
            self.advance();
            return true;
        }
        while let Some(character) = self.current() {
            match character {
                '\'' => {
                    self.advance();
                    return true;
                }
                '/' => return false,
                '\n' if !self.remaining().starts_with("\n'") => return false,
                '\\' => {
                    self.advance();
                    self.advance();
                }
                _ => self.advance(),
            }
        }
        false
    }
}

/// The value of digits written in `radix`, with any underscores among them.
/// Digits are gathered into machine words first, so that a long literal costs
/// one multiplication per word and not one per digit.
fn natural(digits: &str, radix: u32) -> Natural {
    let mut value = Natural::zero();
    let (mut chunk, mut scale) = (0u64, 1u64);
    for digit in digits.chars().filter_map(|digit| digit.to_digit(radix)) {
        if scale > u64::MAX / u64::from(radix) {
            value = value.mul(&Natural::from(scale)).add(&Natural::from(chunk));
            (chunk, scale) = (0, 1);
        }
        // `chunk < scale`, so neither product passes `u64::MAX`.
        chunk = chunk * u64::from(radix) + u64::from(digit);
        scale *= u64::from(radix);
    }
    value.mul(&Natural::from(scale)).add(&Natural::from(chunk))
}
