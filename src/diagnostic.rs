//! Compiler-owned diagnostics; the presentation crate is only an adapter.

pub mod explain;
mod json;

use annotate_snippets::{AnnotationKind, Group, Renderer, Snippet};

use crate::source::{SourceMap, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: String,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    pub message: String,
    pub span: Span,
    pub replacement: String,
    pub applicability: Applicability,
}

/// How serious a diagnostic is. An error rejects the file; a warning is
/// printed and the file is still accepted, as rustc's warnings are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
}

/// Evidence considered while explaining a failed proof obligation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsideredFact {
    pub name: Option<String>,
    pub claim: String,
}

/// Optional proof-specific data, kept independently of human-readable notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProofDetails {
    pub claim: Option<String>,
    pub claim_after_computing: Option<String>,
    pub facts_considered: Option<Vec<ConsideredFact>>,
    pub counterexample: Option<String>,
    pub suggested_explicit_form: Option<String>,
}

/// Infrequently used structured context stays behind one pointer so ordinary
/// diagnostics remain small enough to return as errors without boxing callers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticDetails {
    pub suggestions: Vec<Suggestion>,
    pub helps: Vec<String>,
    pub proof: ProofDetails,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: Level,
    pub code: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub details: Box<DiagnosticDetails>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self::new(Level::Error, code, message, span)
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self::new(Level::Warning, code, message, span)
    }

    pub fn is_error(&self) -> bool {
        self.level == Level::Error
    }

    fn new(level: Level, code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            level,
            code,
            message: message.into(),
            labels: vec![Label {
                span,
                message: String::new(),
                primary: true,
            }],
            notes: Vec::new(),
            details: Box::default(),
        }
    }

    pub fn label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
            primary: false,
        });
        self
    }

    pub fn note(mut self, message: impl Into<String>) -> Self {
        self.notes.push(message.into());
        self
    }

    pub fn claim(mut self, claim: impl Into<String>) -> Self {
        self.details.proof.claim = Some(claim.into());
        self
    }

    pub fn help(mut self, message: impl Into<String>) -> Self {
        self.details.helps.push(message.into());
        self
    }

    pub fn suggest(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub fn render(&self, sources: &SourceMap, color: bool) -> String {
        let title = match self.level {
            Level::Error => annotate_snippets::Level::ERROR,
            Level::Warning => annotate_snippets::Level::WARNING,
        };
        let mut group = Group::with_title(title.primary_title(&self.message).id(self.code));
        let mut files = Vec::new();
        for label in &self.labels {
            if !files.contains(&label.span.file) {
                files.push(label.span.file);
            }
        }
        for id in files {
            let source = sources.get(id);
            let mut snippet = Snippet::source(source.text()).path(&source.name).fold(true);
            for label in self.labels.iter().filter(|label| label.span.file == id) {
                let kind = if label.primary {
                    AnnotationKind::Primary
                } else {
                    AnnotationKind::Context
                };
                snippet = snippet.annotation(kind.span(label.span.range()).label(&label.message));
            }
            group = group.element(snippet);
        }
        for note in &self.notes {
            group = group.element(annotate_snippets::Level::NOTE.message(note));
        }
        for help in &self.details.helps {
            group = group.element(annotate_snippets::Level::HELP.message(help));
        }
        for suggestion in &self.suggestions {
            group = group.element(annotate_snippets::Level::HELP.message(&suggestion.message));
        }
        let renderer = if color {
            Renderer::styled()
        } else {
            Renderer::plain()
        };
        renderer.render(&[group]).to_string()
    }
}

/// Sort mapped diagnostics by file name and byte offset, retaining emission
/// order for ties. Call this after SourceBundle has restored original spans.
pub fn sorted<'a>(sources: &SourceMap, diagnostics: &'a [Diagnostic]) -> Vec<&'a Diagnostic> {
    let mut ordered: Vec<_> = diagnostics.iter().collect();
    ordered.sort_by(|left, right| {
        let key = |diagnostic: &Diagnostic| {
            diagnostic
                .labels
                .iter()
                .find(|label| label.primary)
                .map(|label| (sources.get(label.span.file).name.as_str(), label.span.start))
        };
        key(left).cmp(&key(right))
    });
    ordered
}

// Preserve direct access to structured attachments while storing them together.
impl std::ops::Deref for Diagnostic {
    type Target = DiagnosticDetails;
    fn deref(&self) -> &Self::Target {
        &self.details
    }
}
impl std::ops::DerefMut for Diagnostic {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.details
    }
}
