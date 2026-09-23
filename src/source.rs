//! Source text and byte-based locations shared by all compiler phases.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(file: FileId, start: usize, end: usize) -> Self {
        Self { file, start, end }
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }

    pub fn through(self, other: Self) -> Self {
        assert_eq!(
            self.file, other.file,
            "cannot join spans from different files"
        );
        Self::new(
            self.file,
            self.start.min(other.start),
            self.end.max(other.end),
        )
    }

    pub fn at_end(self) -> Self {
        Self::new(self.file, self.end, self.end)
    }
}

#[derive(Debug)]
pub struct SourceFile {
    pub id: FileId,
    pub name: String,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn slice(&self, span: Span) -> Option<&str> {
        (span.file == self.id)
            .then(|| self.text.get(span.range()))
            .flatten()
    }

    /// One-based line and Unicode scalar column, not terminal display width.
    pub fn line_column(&self, offset: usize) -> Option<(usize, usize)> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return None;
        }
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let column = self.text[self.line_starts[line]..offset].chars().count() + 1;
        Some((line + 1, column))
    }
}

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        let id = FileId(self.files.len());
        let text = text.into();
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(offset, _)| offset + 1));
        self.files.push(SourceFile {
            id,
            name: name.into(),
            text,
            line_starts,
        });
        id
    }

    pub fn get(&self, id: FileId) -> &SourceFile {
        &self.files[id.0]
    }
}

/// A compilation unit assembled from explicit library files and one entry
/// file. Checking uses contiguous offsets; presentation maps every location
/// back to the file that supplied it. The entry file is the last segment.
#[derive(Debug)]
pub struct SourceBundle {
    pub file: FileId,
    segments: Vec<(usize, usize, FileId)>,
}
impl SourceBundle {
    pub fn join(sources: &mut SourceMap, files: &[FileId]) -> Self {
        assert!(!files.is_empty(), "a source bundle needs an entry file");
        if files.len() == 1 {
            return Self {
                file: files[0],
                segments: Vec::new(),
            };
        }
        let mut text = String::new();
        let mut segments = Vec::new();
        for &file in files {
            let start = text.len();
            text.push_str(sources.get(file).text());
            segments.push((start, text.len(), file));
            text.push('\n');
        }
        let name = sources.get(*files.last().unwrap()).name.clone();
        let file = sources.add(name, text);
        Self { file, segments }
    }
    pub fn span(&self, span: Span) -> Span {
        if span.file != self.file || self.segments.is_empty() {
            return span;
        }
        let index = self
            .segments
            .partition_point(|(start, _, _)| *start <= span.start)
            .saturating_sub(1);
        let (start, end, file) = self.segments[index];
        // A recovery span crossing an input boundary is attributed to the
        // input where it starts; no displayed range may leave that file.
        Span::new(
            file,
            span.start.min(end) - start,
            span.end.min(end).max(span.start.min(end)) - start,
        )
    }
    pub fn diagnostic(
        &self,
        diagnostic: &crate::diagnostic::Diagnostic,
    ) -> crate::diagnostic::Diagnostic {
        let mut mapped = diagnostic.clone();
        for label in &mut mapped.labels {
            label.span = self.span(label.span);
        }
        for suggestion in &mut mapped.suggestions {
            suggestion.span = self.span(suggestion.span);
        }
        mapped
    }
}
