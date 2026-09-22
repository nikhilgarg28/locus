//! Compiler-owned diagnostics; the presentation crate is only an adapter.

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: Level,
    pub code: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub suggestions: Vec<Suggestion>,
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
            suggestions: Vec::new(),
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
