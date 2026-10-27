//! Rust's own lexer, applied to litmus sources. Annotations are read only from
//! real line comments, never from strings or block comments, and the PR guard
//! compares programs token by token. Doc comments are tokens, because they are
//! attributes that macros see, and so is whether a token touches the next one,
//! because a macro sees that too: `>=` is one operator and `> =` is two, and
//! `stringify!` prints `a+b` and `a + b` differently. Which whitespace, or
//! which plain comment, separates two tokens is invisible to macros.

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};

/// A line comment: its 1-based line, its text from `//`, and whether code
/// precedes it on that line.
pub struct Comment<'a> {
    pub line: usize,
    pub text: &'a str,
    pub after_code: bool,
    /// Whether any code precedes it in the file.
    pub in_header: bool,
}

/// A token of the program itself: anything but whitespace and plain
/// comments. Doc comments count.
pub struct Token<'a> {
    /// The line the token starts on.
    pub line: usize,
    pub text: &'a str,
    /// Whether the next token follows with no whitespace or plain comment
    /// between.
    pub joint: bool,
}

pub struct Lexed<'a> {
    pub comments: Vec<Comment<'a>>,
    pub tokens: Vec<Token<'a>>,
}

pub fn lex(text: &str) -> Lexed<'_> {
    let mut lexed = Lexed {
        comments: Vec::new(),
        tokens: Vec::new(),
    };
    let mut offset = 0;
    let raw: Vec<(TokenKind, &str)> = tokenize(text, FrontmatterAllowed::No)
        .map(|token| {
            let slice = &text[offset..offset + token.len as usize];
            offset += slice.len();
            (token.kind, slice)
        })
        .collect();
    let mut line = 1;
    // The line on which the last code token ended.
    let mut code_ended_on = None;
    for (index, &(kind, slice)) in raw.iter().enumerate() {
        let start = line;
        line += slice.matches('\n').count();
        match kind {
            TokenKind::LineComment { doc_style: None } => lexed.comments.push(Comment {
                line: start,
                text: slice,
                after_code: code_ended_on == Some(start),
                in_header: code_ended_on.is_none(),
            }),
            _ if is_trivia(kind) => {}
            _ => {
                let joint = raw
                    .get(index + 1)
                    .is_some_and(|&(next, _)| !is_trivia(next));
                lexed.tokens.push(Token {
                    line: start,
                    text: slice,
                    joint,
                });
                code_ended_on = Some(line);
            }
        }
    }
    lexed
}

/// Whitespace and plain comments, which separate tokens and are not part of
/// the program.
fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace
            | TokenKind::LineComment { doc_style: None }
            | TokenKind::BlockComment {
                doc_style: None,
                ..
            }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_comments_outside_strings() {
        let text = "\
//~ question: q
let s = \"//~ error: not a comment\"; // a comment
/* //~ error: in a block comment */
let t = r#\"
//~ error: in a raw string
\"#; //~ error: after a raw string
let c = '/'; let l: &'a str = s;
";
        let lexed = lex(text);
        let comments: Vec<(usize, &str, bool, bool)> = lexed
            .comments
            .iter()
            .map(|c| (c.line, c.text, c.after_code, c.in_header))
            .collect();
        assert_eq!(
            comments,
            [
                (1, "//~ question: q", false, true),
                (2, "// a comment", true, false),
                (6, "//~ error: after a raw string", true, false),
            ]
        );
        assert_eq!(lexed.tokens[0].text, "let");
        assert_eq!(lexed.tokens[0].line, 2);
        assert!(lexed.tokens.iter().any(|t| t.text == "'a"));
    }

    #[test]
    fn doc_comments_are_tokens() {
        let lexed = lex("/// one\n//! two\n/** three */\n// plain\nfn f() {}\n");
        let texts: Vec<&str> = lexed.tokens.iter().map(|t| t.text).collect();
        assert_eq!(&texts[..3], ["/// one", "//! two", "/** three */"]);
        assert_eq!(lexed.comments.len(), 1);
        assert_eq!(lexed.comments[0].text, "// plain");
    }

    #[test]
    fn touching_tokens_are_marked() {
        let joint = |text| -> Vec<(String, bool)> {
            lex(text)
                .tokens
                .iter()
                .map(|t| (t.text.to_string(), t.joint))
                .collect()
        };
        assert_eq!(
            joint("a >=b"),
            [
                ("a".into(), false),
                (">".into(), true),
                ("=".into(), true),
                ("b".into(), false)
            ]
        );
        assert_ne!(joint("a >= b"), joint("a > = b"));
        assert_ne!(joint("a+b"), joint("a + b"));
        // Which whitespace or plain comment separates tokens changes nothing.
        assert_eq!(joint("a + b"), joint("a\n    +  b"));
        assert_eq!(joint("a + b"), joint("a /* c */ + b // d\n"));
    }
}
