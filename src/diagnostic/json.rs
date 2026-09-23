//! Schema 1. A diagnostic is a one-element JSON array on one line.
//! This deliberately small serializer has no dependency on compiler syntax.
use std::fmt::Write;

use super::{Applicability, Diagnostic, Level};
use crate::source::{SourceMap, Span};

pub const SCHEMA_VERSION: u32 = 1;

fn string(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{1f}' => write!(out, "\\u{:04x}", u32::from(c)).unwrap(),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn optional(value: Option<&str>) -> String {
    value.map_or_else(|| "null".into(), string)
}
fn array(values: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", values.into_iter().collect::<Vec<_>>().join(","))
}
fn location(sources: &SourceMap, span: Span) -> String {
    let source = sources.get(span.file);
    let start = source.line_column(span.start);
    let end = source.line_column(span.end);
    let number = |value: Option<usize>| value.map_or_else(|| "null".into(), |v| v.to_string());
    format!(
        "\"file\":{},\"byte_start\":{},\"byte_end\":{},\"line_start\":{},\"column_start\":{},\"line_end\":{},\"column_end\":{}",
        string(&source.name),
        span.start,
        span.end,
        number(start.map(|v| v.0)),
        number(start.map(|v| v.1)),
        number(end.map(|v| v.0)),
        number(end.map(|v| v.1)),
    )
}
impl Diagnostic {
    /// Render schema-versioned JSON without a trailing newline. All optional
    /// proof fields are present; unavailable information is null, not omitted.
    pub fn render_json(&self, sources: &SourceMap) -> String {
        let spans = array(self.labels.iter().map(|label| {
            format!(
                "{{{},\"primary\":{},\"label\":{}}}",
                location(sources, label.span),
                label.primary,
                string(&label.message)
            )
        }));
        let notes = array(self.notes.iter().map(|note| string(note)));
        let helps = array(self.details.helps.iter().map(|help| string(help)));
        let suggestions = array(self.suggestions.iter().map(|suggestion| {
            format!(
                "{{\"message\":{},\"span\":{{{}}},\"replacement\":{},\"applicability\":{}}}",
                string(&suggestion.message),
                location(sources, suggestion.span),
                string(&suggestion.replacement),
                string(match suggestion.applicability {
                    Applicability::MachineApplicable => "machine_applicable",
                    Applicability::MaybeIncorrect => "maybe_incorrect",
                })
            )
        }));
        let proof = &self.details.proof;
        let facts = proof.facts_considered.as_ref().map_or_else(
            || "null".into(),
            |facts| {
                array(facts.iter().map(|fact| {
                    format!(
                        "{{\"name\":{},\"claim\":{}}}",
                        optional(fact.name.as_deref()),
                        string(&fact.claim)
                    )
                }))
            },
        );
        format!(
            "[{{\"schema_version\":{SCHEMA_VERSION},\"severity\":{},\"code\":{},\"message\":{},\"spans\":{spans},\"notes\":{notes},\"helps\":{helps},\"suggestions\":{suggestions},\"claim\":{},\"claim_after_computing\":{},\"facts_considered\":{facts},\"counterexample\":{},\"suggested_explicit_form\":{}}}]",
            string(match self.level {
                Level::Error => "error",
                Level::Warning => "warning",
            }),
            string(self.code),
            string(&self.message),
            optional(proof.claim.as_deref()),
            optional(proof.claim_after_computing.as_deref()),
            optional(proof.counterexample.as_deref()),
            optional(proof.suggested_explicit_form.as_deref()),
        )
    }
}
