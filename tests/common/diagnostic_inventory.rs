//! Shared source-diagnostic inventory, including preview lifecycle rules.
use std::{collections::BTreeSet, path::Path};

/// Every `"L0123"` in a Rust source text: a string literal that is exactly an
/// error code.
///
/// This is a scan of the text and knows nothing of Rust. It misses a code
/// that is not written whole in one literal under `src/`: one assembled with
/// `format!` or `concat!`, or one that comes from another crate. It also
/// takes any literal of this shape for a code, in a comment or in a test as
/// much as in a call to `Diagnostic::error`; that errs towards asking for a
/// rejected file, which is the safe side.
pub fn codes_in(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut codes = BTreeSet::new();
    for start in 0..bytes.len().saturating_sub(6) {
        let window = &bytes[start..start + 7];
        if window[0] == b'"'
            && window[1] == b'L'
            && window[2..6].iter().all(u8::is_ascii_digit)
            && window[6] == b'"'
        {
            codes.insert(text[start + 1..start + 6].to_string());
        }
    }
    codes
}

/// Preview-gate diagnostics have a registry-controlled lifecycle. When every
/// feature is stable no source construct can produce the gate error; driver
/// tests instead verify that old flags are rejected. Only the gate module's
/// single, checked inventory may be inactive. Other source diagnostics still
/// require corpus coverage, even if they happen to use the same code.
pub fn inventory_codes(text: &str, gate_module: bool, active_previews: bool) -> BTreeSet<String> {
    let codes = codes_in(text);
    if gate_module {
        assert_eq!(
            codes,
            BTreeSet::from(["L0255".to_string()]),
            "preview module inventory changed; review lifecycle coverage"
        );
        if !active_previews {
            return BTreeSet::new();
        }
    }
    codes
}

/// Source diagnostics that can be reached with the current feature registry.
pub fn source_codes(path: &Path, text: &str) -> BTreeSet<String> {
    if path.ends_with("src/diagnostic/explain.rs") {
        return BTreeSet::new();
    }
    let active_previews = locus::preview::Feature::ALL
        .into_iter()
        .any(|feature| feature.status() == locus::preview::Status::Preview);
    inventory_codes(text, path.ends_with("src/preview.rs"), active_previews)
        .into_iter()
        .filter(|code| !code.starts_with("L04") && code != "L0010" && code != "L0011")
        .collect()
}
