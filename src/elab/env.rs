//! What is in scope: declared items, local names, and known facts.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::diagnostic::Diagnostic;
use crate::kernel::theory::Theory;
use crate::kernel::{
    Context, EnumId, FnId, HypId, KernelError, Mode, Prelude, Proof, PropId, StructId, Term, Type,
    VarId, infer_term, telescope_entry,
};
use crate::source::{SourceFile, Span};
use crate::typed::{Binder, FnRef, Session};

use super::items::HoleReport;

pub(super) type Elab<T> = Result<T, ()>;

#[derive(Debug)]
pub(super) struct StructInfo {
    pub id: StructId,
    pub name: String,
    /// A field's type may mention the binders of the fields before it.
    pub fields: Vec<Binder>,
}

#[derive(Debug)]
pub(super) struct EnumInfo {
    pub id: EnumId,
    pub name: String,
    pub variants: Vec<(String, Vec<Binder>)>,
}

#[derive(Debug)]
pub(super) struct PropInfo {
    pub id: PropId,
    pub name: String,
    pub params: Vec<Type>,
}

#[derive(Debug)]
pub(super) struct FnInfo {
    pub reference: FnRef,
    pub name: String,
    pub params: Vec<Binder>,
    /// Over the parameters' identities.
    pub result: Type,
}

#[derive(Clone, Debug)]
pub(super) enum Global {
    Struct(Rc<StructInfo>),
    Enum(Rc<EnumInfo>),
    #[allow(dead_code)]
    Prop(Rc<PropInfo>),
    Fn(Rc<FnInfo>),
}

#[derive(Clone, Debug)]
pub(super) struct Local {
    pub name: String,
    pub id: VarId,
    pub ty: Type,
    /// The binding failed to elaborate. A use of it is a consequence of an
    /// error already reported, and is not reported again.
    pub poisoned: bool,
}

/// Something known at this point, with the proof that it holds.
#[derive(Clone, Debug)]
pub(super) struct Fact {
    pub proof: Proof,
    pub claim: Term,
}

/// The loop a `break` or `continue` belongs to.
#[derive(Clone, Debug)]
pub(super) struct LoopTarget {
    /// The state binders, in order.
    pub state: Vec<Binder>,
    /// What `continue` substitutes for the index of a `for`.
    pub advance: Option<(VarId, Term)>,
    /// The type `break` produces; absent in a `for`.
    pub result: Option<Type>,
}

/// A point to return to at the end of a lexical scope.
pub(super) struct Mark {
    ctx: crate::kernel::Checkpoint,
    names: usize,
    facts: usize,
}

pub(super) struct Env<'a> {
    pub source: &'a SourceFile,
    pub session: Session,
    pub prelude: Prelude,
    #[allow(dead_code)]
    pub theory: Theory,
    pub globals: HashMap<String, Global>,
    /// Items that were rejected; a mention of one is not reported again.
    pub failed: HashSet<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub holes: Vec<HoleReport>,

    // The function being elaborated.
    pub ctx: Context,
    pub names: Vec<Local>,
    pub facts: Vec<Fact>,
    pub loops: Vec<LoopTarget>,
    /// How to print each identity: a name, or the source text of the
    /// expression whose result it is.
    pub labels: HashMap<VarId, String>,
    /// Inside a `math fn`, a proposition, or a proof type, where nothing may
    /// fail to return.
    pub total: bool,
}

impl Env<'_> {
    pub fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }

    pub fn fail<T>(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
    ) -> Elab<T> {
        self.error(code, message, span);
        Err(())
    }

    /// A kernel rejection of something the elaborator built. The user did
    /// nothing wrong that an earlier, better message should not have caught.
    pub fn internal<T>(&mut self, error: impl std::fmt::Display, span: Span) -> Elab<T> {
        self.diagnostics.push(
            Diagnostic::error("L0299", format!("the checker rejected this: {error}"), span)
                .note("this is a gap in the elaborator's own checks; the program was not accepted"),
        );
        Err(())
    }

    pub fn kernel<T>(&mut self, result: Result<T, KernelError>, span: Span) -> Elab<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => self.internal(error, span),
        }
    }

    pub fn text(&self, span: Span) -> &str {
        self.source.slice(span).unwrap_or("")
    }

    pub fn mark(&self) -> Mark {
        Mark {
            ctx: self.ctx.checkpoint(),
            names: self.names.len(),
            facts: self.facts.len(),
        }
    }

    /// Ends a scope whose bindings and facts end with it: a branch, an arm,
    /// a loop body, a quantifier.
    pub fn close(&mut self, mark: Mark) {
        self.ctx.rollback(mark.ctx);
        self.names.truncate(mark.names);
        self.facts.truncate(mark.facts);
    }

    /// Ends a block whose statements the checker splices into the enclosing
    /// sequence: the names and facts go out of scope, and the identities they
    /// spoke of remain declared.
    pub fn close_names(&mut self, mark: Mark) {
        self.names.truncate(mark.names);
        self.facts.truncate(mark.facts);
    }

    pub fn lookup(&self, name: &str) -> Option<&Local> {
        self.names.iter().rev().find(|local| local.name == name)
    }

    pub fn type_of(&mut self, term: &Term, span: Span) -> Elab<Type> {
        let result = infer_term(&mut self.ctx, term, Mode::Logical);
        self.kernel(result, span)
    }

    /// Makes `name` refer to an identity the context already holds, and
    /// records what its type lets one conclude.
    pub fn bind(&mut self, name: &str, id: VarId, ty: &Type) {
        self.labels.insert(id, name.to_string());
        self.names.push(Local {
            name: name.to_string(),
            id,
            ty: ty.clone(),
            poisoned: false,
        });
        self.learn_from(&Term::var(id), ty);
    }

    /// Brings the names of a pattern into scope after its `let` failed.
    pub fn poison(&mut self, pattern: &crate::ast::Pattern) {
        use crate::ast::PatternKind;
        match &pattern.kind {
            PatternKind::Name(name) => self.names.push(Local {
                name: name.text.clone(),
                id: VarId::fresh(),
                ty: Type::Tuple(Vec::new()),
                poisoned: true,
            }),
            PatternKind::Group(inner) => self.poison(inner),
            PatternKind::Tuple(parts) => parts.iter().for_each(|part| self.poison(part)),
            PatternKind::Struct { fields, .. } => {
                fields.iter().for_each(|field| self.poison(&field.pattern));
            }
            PatternKind::Variant { arguments, .. } => {
                arguments
                    .iter()
                    .flatten()
                    .for_each(|part| self.poison(part));
            }
            _ => {}
        }
    }

    /// Declares a variable in the mirrored context and brings it into scope.
    pub fn declare(&mut self, binder: &Binder, ghost: bool, span: Span) -> Elab<()> {
        let result = self.ctx.declare_with(binder.id, binder.ty.clone(), ghost);
        self.kernel(result, span)?;
        self.bind(&binder.name, binder.id, &binder.ty);
        Ok(())
    }

    /// Declares the result of an expression that the checker will name.
    pub fn declare_result(&mut self, id: VarId, ty: &Type, span: Span) -> Elab<()> {
        let result = self.ctx.declare_with(id, ty.clone(), false);
        self.kernel(result, span)?;
        let label = self
            .text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.labels.insert(id, label);
        self.learn_from(&Term::var(id), ty);
        Ok(())
    }

    pub fn assume(&mut self, id: HypId, claim: Term, span: Span) -> Elab<()> {
        let result = self.ctx.assume_with(id, claim.clone());
        self.kernel(result, span)?;
        self.facts.push(Fact {
            proof: Proof::hyp(id),
            claim,
        });
        Ok(())
    }

    /// The facts a value carries: itself when it is a proof, and the proof
    /// fields of a tuple or struct, each about the value's own fields.
    fn learn_from(&mut self, value: &Term, ty: &Type) {
        match ty {
            Type::Proof(claim) => self.facts.push(Fact {
                proof: Proof::OfTerm(value.clone()),
                claim: (**claim).clone(),
            }),
            Type::Tuple(fields) => {
                let earlier: Vec<Term> = (0..fields.len())
                    .map(|index| Term::proj(value.clone(), index))
                    .collect();
                for index in 0..fields.len() {
                    if let Some(field) = telescope_entry(ty, index, &earlier[..index]) {
                        self.learn_from(&earlier[index], &field);
                    }
                }
            }
            Type::Struct(id) => {
                let Some(info) = self.struct_by_id(*id) else {
                    return;
                };
                for (index, field) in info.fields.iter().enumerate() {
                    let mut field_ty = field.ty.clone();
                    for (earlier, binder) in info.fields[..index].iter().enumerate() {
                        field_ty =
                            field_ty.replace_var(binder.id, &Term::proj(value.clone(), earlier));
                    }
                    self.learn_from(&Term::proj(value.clone(), index), &field_ty);
                }
            }
            _ => {}
        }
    }

    pub fn struct_by_id(&self, id: StructId) -> Option<Rc<StructInfo>> {
        self.globals.values().find_map(|global| match global {
            Global::Struct(info) if info.id == id => Some(Rc::clone(info)),
            _ => None,
        })
    }

    pub fn enum_by_id(&self, id: EnumId) -> Option<Rc<EnumInfo>> {
        self.globals.values().find_map(|global| match global {
            Global::Enum(info) if info.id == id => Some(Rc::clone(info)),
            _ => None,
        })
    }

    pub fn fn_by_id(&self, id: FnId) -> Option<Rc<FnInfo>> {
        self.globals.values().find_map(|global| match global {
            Global::Fn(info) if info.reference == FnRef::Math(id) => Some(Rc::clone(info)),
            _ => None,
        })
    }
}
