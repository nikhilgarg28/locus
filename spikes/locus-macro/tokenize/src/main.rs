//! Question 1, with proc_macro2's own lexer (its fallback, since this is not
//! a procedural macro): does each file's full text, comments included,
//! tokenize as a Rust token stream?
//!
//!     tokenize OUT_DIR ROOT FILE...
//!
//! For each FILE that tokenizes, writes its canonical dump (see
//! `locus-macros/src/dump.rs`) to OUT_DIR/<path under ROOT, with `/` as `__`>
//! `.fallback.txt`, to compare with the dump of rustc's tokens.
use std::{fs, path::Path, str::FromStr};

#[path = "../../locus-macros/src/dump.rs"]
mod dump;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = Path::new(&args[1]);
    let root = Path::new(&args[2]);
    let (mut ok, mut failed) = (0, 0);
    for f in &args[3..] {
        let rel = Path::new(f).strip_prefix(root).unwrap_or(Path::new(f));
        let flat = rel.to_string_lossy().replace('/', "__");
        let text = fs::read_to_string(f).expect("read");
        match proc_macro2::TokenStream::from_str(&text) {
            Ok(ts) => {
                ok += 1;
                fs::write(out.join(format!("{flat}.fallback.txt")), dump::dump(ts)).expect("write");
            }
            Err(e) => {
                failed += 1;
                let at = e.span().start();
                let line = text.lines().nth(at.line.saturating_sub(1)).unwrap_or("");
                println!(
                    "  fail {}: {e} (line {}, column {}): {}",
                    rel.display(),
                    at.line,
                    at.column + 1,
                    line.trim()
                );
            }
        }
    }
    println!(
        "proc_macro2 fallback lexer: {ok} of {} tokenize, {failed} fail",
        ok + failed
    );
}
