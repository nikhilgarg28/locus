//! The kernel context: an ordered list of executable variables, ghost
//! variables, and hypotheses.

use super::check::infer_term;
use super::error::KernelError;
use super::term::{HypId, Term, Type, VarId};

/// How a term is being used. `Logical` is the upgraded reading of the
/// context, in which ghost variables are ordinary variables. `Executable`
/// means the term's value is required at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Executable,
    Logical,
}

#[derive(Clone, Debug)]
enum Entry {
    Var { id: VarId, ty: Type, ghost: bool },
    Hyp { id: HypId, prop: Term },
}

#[derive(Clone, Debug, Default)]
pub struct Context {
    entries: Vec<Entry>,
}

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares an executable variable. A variable of a ghost type is ghost
    /// however it is declared.
    pub fn declare(&mut self, ty: Type) -> VarId {
        let ghost = ty.is_ghost();
        self.push_var(ty, ghost)
    }

    pub fn declare_ghost(&mut self, ty: Type) -> VarId {
        self.push_var(ty, true)
    }

    /// Adds a hypothesis. The proposition must be well formed here.
    pub fn assume(&mut self, prop: Term) -> Result<HypId, KernelError> {
        let ty = infer_term(self, &prop, Mode::Logical)?;
        if ty != Type::Prop {
            return Err(KernelError::TypeMismatch {
                expected: Type::Prop,
                found: ty,
            });
        }
        Ok(self.push_hyp(prop))
    }

    fn push_var(&mut self, ty: Type, ghost: bool) -> VarId {
        let id = VarId::fresh();
        self.entries.push(Entry::Var { id, ty, ghost });
        id
    }

    /// The caller has already checked that `prop` is a proposition.
    pub(super) fn push_hyp(&mut self, prop: Term) -> HypId {
        let id = HypId::fresh();
        self.entries.push(Entry::Hyp { id, prop });
        id
    }

    pub(super) fn push_bound(&mut self, ty: Type) -> VarId {
        self.push_var(ty, true)
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Leaves the scope of everything pushed since `len` was read.
    pub(super) fn truncate(&mut self, len: usize) {
        self.entries.truncate(len);
    }

    /// Returns the variable's type and whether it is ghost.
    pub(super) fn var(&self, target: VarId) -> Option<(&Type, bool)> {
        self.entries.iter().rev().find_map(|entry| match entry {
            Entry::Var { id, ty, ghost } if *id == target => Some((ty, *ghost)),
            _ => None,
        })
    }

    pub(super) fn hyp(&self, target: HypId) -> Option<&Term> {
        self.entries.iter().rev().find_map(|entry| match entry {
            Entry::Hyp { id, prop } if *id == target => Some(prop),
            _ => None,
        })
    }
}
