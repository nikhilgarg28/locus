//! Checks the calculus inventory against actual check-IR variants.
use std::collections::BTreeSet;

fn variants(source: &str, name: &str) -> BTreeSet<String> {
    let start = source.find(&format!("pub enum {name} {{")).unwrap();
    let body = &source[start..];
    let mut depth = 0i32;
    let mut names = BTreeSet::new();
    for line in body.lines() {
        let text = line.trim();
        if text.starts_with("//") {
            continue;
        }
        if depth == 1 && text.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
            names.insert(
                text.split(|c: char| !c.is_ascii_alphanumeric())
                    .next()
                    .unwrap()
                    .to_string(),
            );
        }
        depth += text.chars().filter(|&c| c == '{').count() as i32;
        depth -= text.chars().filter(|&c| c == '}').count() as i32;
        if depth == 0 {
            break;
        }
    }
    names
}

#[test]
#[doc = "spec: 3.1:2"]
fn calculus_grammar_covers_every_check_ir_constructor() {
    let result = std::process::Command::new("python3")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["-c", "import sys; sys.path.insert(0, 'tools'); import spec; data = spec.load(spec.ROOT / 'docs'); print('\\n'.join(next(d['body'] for d in data['docs'] if d['id'] == 'formal-core')))"])
        .output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let atlas = String::from_utf8(result.stdout).unwrap();
    let ir = include_str!("../src/exec/ir.rs");
    let native = include_str!("../src/kernel/buffer.rs");
    for (source, name, expected) in [
        (
            ir,
            "Stmt",
            "Let Have Call Match Loop For Operate Buffer BoxNew",
        ),
        (ir, "Tail", "Value Break Continue Match Return Panic"),
        (ir, "BufferStorage", "Array Slice Vector"),
        (native, "BufferOp", "Literal Length Get Set Push"),
    ] {
        let written: BTreeSet<_> = expected.split_whitespace().map(String::from).collect();
        assert_eq!(
            variants(source, name),
            written,
            "new {name} form requires calculus rule and test"
        );
        for variant in written {
            assert!(atlas.contains(&variant), "calculus omits {name}::{variant}");
        }
    }
    for rule in [
        "IR-Bind",
        "IR-Call",
        "IR-Match",
        "IR-Loop",
        "IR-For",
        "IR-End",
        "IR-Operate",
        "IR-Storage",
        "IR-Promises",
        "L-Eval",
        "L-Write",
        "L-Tracked",
        "L-Join",
        "L-Iteration",
        "L-Lend",
        "L-Permission",
        "E-Type",
        "E-Expr",
        "E-Clean/Export",
    ] {
        assert!(atlas.contains(rule), "calculus omits {rule}");
    }
}
