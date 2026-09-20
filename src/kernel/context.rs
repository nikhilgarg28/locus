//! The kernel context: an ordered list of executable variables, ghost
//! variables, and hypotheses.

use std::rc::Rc;

use super::check::{check_type, infer_term};
use super::defs::Definitions;
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
    definitions: Rc<Definitions>,
    entries: Vec<Entry>,
}

impl Context {
    /// An empty context over no declarations.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_definitions(definitions: Rc<Definitions>) -> Self {
        Self {
            definitions,
            entries: Vec::new(),
        }
    }

    pub(super) fn definitions(&self) -> Rc<Definitions> {
        Rc::clone(&self.definitions)
    }

    /// Declares an executable variable. The type must be well formed here.
    /// A variable of a ghost type is ghost however it is declared.
    pub fn declare(&mut self, ty: Type) -> Result<VarId, KernelError> {
        check_type(self, &ty)?;
        let ghost = ty.is_ghost();
        Ok(self.push_var(ty, ghost))
    }

    pub fn declare_ghost(&mut self, ty: Type) -> Result<VarId, KernelError> {
        check_type(self, &ty)?;
        Ok(self.push_var(ty, true))
    }

    /// An immutable logical `let`: declares `x` and assumes `x == value`.
    /// This is `declare` followed by `assume` and adds no rule of its own;
    /// the equation is how the let computation axiom reaches the kernel.
    /// The variable is executable when `value` is an executable term.
    pub fn define(&mut self, value: &Term) -> Result<(VarId, HypId), KernelError> {
        let ghost = infer_term(self, value, Mode::Executable).is_err();
        let ty = infer_term(self, value, Mode::Logical)?;
        let var = self.push_var(ty.clone(), ghost || ty.is_ghost());
        let equation = Term::eq(ty, Term::Free(var), value.clone());
        match self.assume(equation) {
            Ok(hyp) => Ok((var, hyp)),
            Err(error) => {
                self.entries.pop();
                Err(error)
            }
        }
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

    /// A variable bound by an executable construct, such as a case arm.
    pub(super) fn push_local(&mut self, ty: Type, ghost: bool) -> VarId {
        self.push_var(ty, ghost)
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
