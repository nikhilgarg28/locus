//! Rust's own lexer, applied to litmus sources. Annotations are read only from
//! real line comments, never from strings or block comments, and the PR guard
//! compares programs token by token.

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

/// A token of the program itself: anything but whitespace and comments.
pub struct Token<'a> {
    /// The line the token starts on.
    pub line: usize,
    pub text: &'a str,
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
    let mut line = 1;
    let mut offset = 0;
    // The line on which the last code token ended.
    let mut code_ended_on = None;
    for token in tokenize(text, FrontmatterAllowed::No) {
        let slice = &text[offset..offset + token.len as usize];
        offset += slice.len();
        let start = line;
        line += slice.matches('\n').count();
        match token.kind {
            TokenKind::Whitespace | TokenKind::BlockComment { .. } => {}
            TokenKind::LineComment { .. } => lexed.comments.push(Comment {
                line: start,
                text: slice,
                after_code: code_ended_on == Some(start),
                in_header: code_ended_on.is_none(),
            }),
            _ => {
                lexed.tokens.push(Token {
                    line: start,
                    text: slice,
                });
                code_ended_on = Some(line);
            }
        }
    }
    lexed
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
}
