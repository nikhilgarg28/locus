//! Closed, task-owned gates for features that have not reached acceptance.
//!
//! Parsing is independent of these gates. Elaboration requires a feature at
//! the construct that uses it, so unfinished syntax has a useful diagnostic.

use std::collections::BTreeSet;
use std::fmt;

use crate::diagnostic::Diagnostic;
use crate::source::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    LogicalSplit,
    NamedProps,
    LogicalData,
    HeapViews,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Preview,
    Stabilized,
}

impl Feature {
    pub const ALL: [Self; 4] = [
        Self::LogicalSplit,
        Self::NamedProps,
        Self::LogicalData,
        Self::HeapViews,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::LogicalSplit => "logical-split",
            Self::NamedProps => "named-props",
            Self::LogicalData => "logical-data",
            Self::HeapViews => "heap-views",
        }
    }

    /// The Atlas task that owns introduction of this gate.
    pub const fn task(self) -> &'static str {
        match self {
            Self::LogicalSplit => "LOC-209",
            Self::NamedProps => "LOC-206",
            Self::LogicalData => "LOC-218",
            Self::HeapViews => "LOC-228",
        }
    }

    /// Acceptance stabilizes a feature here and removes corpus directives.
    /// Keep the name, so an obsolete flag is distinguished from a typo.
    pub const fn status(self) -> Status {
        match self {
            Self::LogicalSplit | Self::NamedProps | Self::LogicalData | Self::HeapViews => {
                Status::Stabilized
            }
        }
    }

    pub fn parse(name: &str) -> Result<Self, PreviewOptionsError> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.name() == name)
            .ok_or_else(|| PreviewOptionsError::Unknown(name.into()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewOptionsError {
    Unknown(String),
    Stabilized(Feature),
}

impl fmt::Display for PreviewOptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(name) => write!(
                f,
                "unknown preview feature `{name}`; known features: {}",
                Feature::ALL.map(Feature::name).join(", ")
            ),
            Self::Stabilized(feature) => write!(
                f,
                "preview feature `{}` is stabilized; remove its preview flag or directive",
                feature.name()
            ),
        }
    }
}

impl std::error::Error for PreviewOptionsError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Previews {
    enabled: BTreeSet<Feature>,
}

impl Previews {
    pub fn enable(&mut self, name: &str) -> Result<(), PreviewOptionsError> {
        let feature = Feature::parse(name)?;
        if feature.status() == Status::Stabilized {
            return Err(PreviewOptionsError::Stabilized(feature));
        }
        self.enabled.insert(feature);
        Ok(())
    }

    pub fn contains(&self, feature: Feature) -> bool {
        feature.status() == Status::Stabilized || self.enabled.contains(&feature)
    }

    pub fn is_empty(&self) -> bool {
        self.enabled.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = Feature> + '_ {
        self.enabled.iter().copied()
    }

    pub fn require(&self, feature: Feature, construct: &str, span: Span) -> Result<(), Diagnostic> {
        if self.contains(feature) {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "L0255",
                format!("{construct} requires preview feature `{}`", feature.name()),
                span,
            )
            .note(format!(
                "enable with `--preview {}`; this unfinished feature is owned by {}",
                feature.name(),
                feature.task()
            )))
        }
    }
}
