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
