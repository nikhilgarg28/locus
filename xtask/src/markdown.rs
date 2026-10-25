//! The small amount of Markdown structure the checks need: fenced code blocks,
//! titles, and inline links. This is deliberately not a full parser.

/// A line of a Markdown file, with its 1-based number and whether it lies
/// inside a fenced code block (fence lines themselves count as inside).
pub struct Line<'a> {
    pub number: usize,
    pub text: &'a str,
    pub in_fence: bool,
}

pub fn lines(text: &str) -> Vec<Line<'_>> {
    let mut open: Option<(char, usize)> = None;
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let fence = fence_marker(line);
        let in_fence = match (open, fence) {
            (None, Some((ch, len, _))) => {
                open = Some((ch, len));
                true
            }
            (Some((ch, len)), Some((c, l, info))) if c == ch && l >= len && info.is_empty() => {
                open = None;
                true
            }
            (Some(_), _) => true,
            (None, None) => false,
        };
        out.push(Line {
            number: index + 1,
            text: line,
            in_fence,
        });
    }
    out
}

/// A fenced code block: the line of its opening fence, its info string, and
/// its body.
pub struct Fence {
    pub line: usize,
    pub info: String,
    pub body: String,
}

pub fn fences(text: &str) -> Vec<Fence> {
    let mut out: Vec<Fence> = Vec::new();
    let mut open: Option<(char, usize)> = None;
    for (index, line) in text.lines().enumerate() {
        match (open, fence_marker(line)) {
            (None, Some((ch, len, info))) => {
                open = Some((ch, len));
                out.push(Fence {
                    line: index + 1,
                    info: info.to_string(),
                    body: String::new(),
                });
            }
            (Some((ch, len)), Some((c, l, info))) if c == ch && l >= len && info.is_empty() => {
                open = None;
            }
            (Some(_), _) => {
                let fence = out.last_mut().expect("an open fence was pushed");
                fence.body.push_str(line);
                fence.body.push('\n');
            }
            (None, None) => {}
        }
    }
    out
}

/// If `line` is a fence (three or more backticks or tildes, indented by at
/// most three spaces), returns its character, length and info string.
fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let ch = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = trimmed.chars().take_while(|c| *c == ch).count();
    if len < 3 {
        return None;
    }
    let info = trimmed[len..].trim();
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some((ch, len, info))
}

/// The text of the first `# ` heading.
pub fn title(text: &str) -> Option<&str> {
    lines(text)
        .into_iter()
        .filter(|l| !l.in_fence)
        .find_map(|l| l.text.strip_prefix("# "))
        .map(str::trim)
}

/// Inline link targets `[text](target)` outside code, with their line numbers.
pub fn links(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for line in lines(text) {
        if line.in_fence {
            continue;
        }
        let prose = strip_code_spans(line.text);
        let mut rest = prose.as_str();
        while let Some(start) = rest.find("](") {
            let after = &rest[start + 2..];
            match after.find(')') {
                Some(end) => {
                    let inner = &after[..end];
                    let target = match inner.strip_prefix('<') {
                        Some(bracketed) => bracketed.split('>').next().unwrap_or(""),
                        None => inner.split_whitespace().next().unwrap_or(""),
                    };
                    out.push((line.number, target.to_string()));
                    rest = &after[end + 1..];
                }
                None => break,
            }
        }
    }
    out
}

/// Rewrites the target of each inline link outside code. `rewrite` returns
/// the new target, or `None` to keep the link as it is.
pub fn rewrite_links(text: &str, mut rewrite: impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(text.len());
    for line in lines(text) {
        if line.in_fence {
            out.push_str(line.text);
        } else {
            out.push_str(&rewrite_line(line.text, &mut rewrite));
        }
        out.push('\n');
    }
    out
}

fn rewrite_line(line: &str, rewrite: &mut impl FnMut(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    let mut i = 0;
    while let Some(ch) = line[i..].chars().next() {
        let rest = &line[i..];
        if ch == '`' {
            in_code = !in_code;
        }
        if !in_code
            && rest.starts_with("](")
            && let Some(end) = rest[2..].find(')')
        {
            let inner = &rest[2..2 + end];
            let (open, target, close) =
                match inner.strip_prefix('<').and_then(|s| s.split_once('>')) {
                    Some((target, after)) => ("<", target, format!(">{after}")),
                    None => {
                        let target = inner.split_whitespace().next().unwrap_or("");
                        let after = inner
                            .find(target)
                            .map_or("", |at| &inner[at + target.len()..]);
                        ("", target, after.to_string())
                    }
                };
            let new = rewrite(target).unwrap_or_else(|| target.to_string());
            out.push_str(&format!("]({open}{new}{close})"));
            i += 2 + end + 1;
            continue;
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Replaces the contents of inline code spans with spaces, so that text such
/// as `` `[a](b)` `` is not mistaken for a link.
pub fn strip_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            out.push(ch);
        } else if in_code {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

/// Whether a link target points outside the repository.
pub fn is_external(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://") || target.starts_with("mailto:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_fences() {
        let text = "a\n```rust check\nfn f() {}\n```\nb\n~~~~\n```\n~~~~\n";
        let inside: Vec<bool> = lines(text).iter().map(|l| l.in_fence).collect();
        assert_eq!(inside, [false, true, true, true, false, true, true, true]);
        let fences = fences(text);
        assert_eq!(fences.len(), 2);
        assert_eq!(fences[0].info, "rust check");
        assert_eq!(fences[0].body, "fn f() {}\n");
        assert_eq!(fences[1].body, "```\n");
    }

    #[test]
    fn finds_links_outside_code() {
        let text = "See [a](a.md#x) and `[b](b.md)`.\n```\n[c](c.md)\n```\n[d](<d.md> \"t\")";
        let links = links(text);
        assert_eq!(links, [(1, "a.md#x".to_string()), (5, "d.md".to_string())]);
    }

    #[test]
    fn rewrites_links_outside_code() {
        let text = "[a](a.md \"t\") `[b](b.md)` [c](<c d.md>)\n```\n[e](e.md)\n```\n";
        let rewritten = rewrite_links(text, |t| Some(format!("x/{t}")));
        assert_eq!(
            rewritten,
            "[a](x/a.md \"t\") `[b](b.md)` [c](<x/c d.md>)\n```\n[e](e.md)\n```\n"
        );
    }

    #[test]
    fn finds_title() {
        assert_eq!(
            title("```\n# not\n```\n# RFC 0001: X\n"),
            Some("RFC 0001: X")
        );
    }
}
