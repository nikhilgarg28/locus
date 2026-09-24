//! A one-way projection of an ordinary return value for safe Rust callers.
//! Only proof leaves and tuple structure are changed. In particular a nominal
//! type is never rebuilt, and a runtime enum tag is never erased.
use super::{EBlock, EExpr, EFn, EPattern, EPlace, EStmt, EType};
use crate::{kernel::VarId, typed::Passing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProofOutput {
    Keep(EType),
    Omit,
    Tuple(Vec<Self>),
}

impl ProofOutput {
    pub(crate) fn new(ty: &EType) -> Self {
        match ty {
            EType::Proved => Self::Omit,
            EType::Tuple(fields) if !fields.is_empty() => {
                let parts: Vec<_> = fields.iter().map(Self::new).collect();
                if parts.iter().all(Self::omitted) {
                    Self::Omit
                } else if parts.iter().any(Self::changed) {
                    Self::Tuple(parts)
                } else {
                    Self::Keep(ty.clone())
                }
            }
            _ => Self::Keep(ty.clone()),
        }
    }

    pub(crate) fn changed(&self) -> bool {
        !matches!(self, Self::Keep(_))
    }

    fn omitted(&self) -> bool {
        matches!(self, Self::Omit)
    }

    fn unwraps(&self) -> bool {
        matches!(self, Self::Tuple(parts) if parts.iter().any(Self::omitted)
            && parts.iter().filter(|p| !p.omitted()).count() == 1)
    }

    pub(crate) fn result(&self) -> EType {
        match self {
            Self::Keep(ty) => ty.clone(),
            Self::Omit => EType::unit(),
            Self::Tuple(parts) => {
                let mut fields: Vec<_> = parts
                    .iter()
                    .filter(|p| !p.omitted())
                    .map(Self::result)
                    .collect();
                if self.unwraps() {
                    fields.remove(0)
                } else {
                    EType::Tuple(fields)
                }
            }
        }
    }

    /// Reserved names keep private proof-returning methods distinct from the
    /// public method, including calls made recursively or through another method.
    pub(crate) fn implementation(name: &str) -> String {
        match name.rsplit_once("::") {
            Some((owner, method)) => {
                let trimmed = method.trim_start_matches('_');
                let method = if trimmed.len() == method.len() {
                    method.to_owned()
                } else {
                    format!("{}_{}", method.len() - trimmed.len(), trimmed)
                };
                format!("{owner}::__locus_with_proofs_{method}")
            }
            None => format!(
                "__locus_with_proofs_{}",
                name.strip_prefix("__locus_").unwrap_or(name)
            ),
        }
    }

    pub(crate) fn wrapper(&self, original: &EFn) -> EFn {
        let mut function = original.clone();
        function.result = self.result();
        function.passing = original
            .params
            .iter()
            .enumerate()
            .map(|(i, _)| match original.passing_of(i) {
                Passing::MutValue => Passing::Value,
                other => other,
            })
            .collect();
        let arguments = original
            .params
            .iter()
            .enumerate()
            .map(|(i, (id, name, _))| {
                if original.passing_of(i).is_reference() {
                    EExpr::Lend {
                        mutable: original.passing_of(i) == Passing::RefMut,
                        place: EPlace {
                            id: *id,
                            name: name.clone(),
                            path: Vec::new(),
                        },
                    }
                } else {
                    EExpr::Var {
                        id: *id,
                        name: name.clone(),
                    }
                }
            })
            .collect();
        let (pattern, value) = self.project(&mut 0);
        function.body = EBlock {
            stmts: vec![EStmt::Let {
                pattern,
                value: EExpr::Call {
                    callee: original.reference,
                    name: Self::implementation(&original.name),
                    arguments,
                },
            }],
            tail: Some(Box::new(value)),
        };
        function
    }

    // Pattern matching moves each surviving physical component exactly once.
    // No runtime data is discarded, cloned, reconstructed or evaluated twice.
    fn project(&self, next: &mut usize) -> (EPattern, EExpr) {
        match self {
            Self::Omit => (EPattern::Wildcard, EExpr::Tuple(Vec::new())),
            Self::Keep(ty) => {
                let id = VarId::fresh();
                let name = format!("__locus_result_{next}");
                *next += 1;
                (
                    EPattern::Bind {
                        id,
                        name: name.clone(),
                        ty: ty.clone(),
                        mutable: false,
                    },
                    EExpr::Var { id, name },
                )
            }
            Self::Tuple(parts) => {
                let mut patterns = Vec::new();
                let mut values = Vec::new();
                for part in parts {
                    let (pattern, value) = part.project(next);
                    patterns.push(pattern);
                    if !part.omitted() {
                        values.push(value);
                    }
                }
                let value = if self.unwraps() {
                    values.remove(0)
                } else {
                    EExpr::Tuple(values)
                };
                (EPattern::Tuple(patterns), value)
            }
        }
    }
}
