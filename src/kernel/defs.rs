//! Global declarations: structs and math functions.
//!
//! A declaration is checked against the declarations that precede it, so a
//! struct cannot mention itself and a function cannot call itself, directly or
//! through other declarations. With no loops in kernel terms, that makes
//! every declared function total.

use std::rc::Rc;

use super::check::{check_telescope, expect_type};
use super::context::{Context, Mode};
use super::error::KernelError;
use super::term::{FnId, StructId, Term, Type, field_type};

#[derive(Clone, Debug)]
pub(super) struct StructDecl {
    /// A telescope, as in `Type::Tuple`.
    pub(super) fields: Vec<Type>,
}

#[derive(Clone, Debug)]
pub(super) struct FnDecl {
    /// A telescope, as in `Type::Fn`.
    pub(super) params: Vec<Type>,
    /// Under all the parameters.
    pub(super) result: Type,
    /// Under all the parameters.
    pub(super) body: Term,
}

#[derive(Clone, Debug, Default)]
pub struct Definitions {
    structs: Vec<StructDecl>,
    fns: Vec<FnDecl>,
}

impl Definitions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a struct with the fields of the given tuple type. The fields
    /// must be well formed with no variables in scope.
    pub fn declare_struct(&mut self, fields: &Type) -> Result<StructId, KernelError> {
        let Type::Tuple(fields) = fields else {
            return Err(KernelError::NotAProduct(fields.clone()));
        };
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        check_telescope(&mut ctx, fields)?;
        self.structs.push(StructDecl {
            fields: fields.clone(),
        });
        Ok(StructId(self.structs.len() - 1))
    }

    /// Declares a math function of the given function type. `body(params)`
    /// receives the parameters as terms. The body is checked against the
    /// result type with no other variables in scope.
    pub fn declare_fn(
        &mut self,
        signature: &Type,
        body: impl FnOnce(&[Term]) -> Term,
    ) -> Result<FnId, KernelError> {
        let Type::Fn(params, result) = signature else {
            return Err(KernelError::NotAFunction(signature.clone()));
        };
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let mut telescope = params.clone();
        telescope.push((**result).clone());
        check_telescope(&mut ctx, &telescope)?;

        let mut vars = Vec::new();
        for index in 0..params.len() {
            let ty = field_type(&telescope, index, |j| Term::Free(vars[j]));
            vars.push(ctx.push_bound(ty));
        }
        let arguments: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
        let body = body(&arguments);
        let expected = field_type(&telescope, params.len(), |j| Term::Free(vars[j]));
        expect_type(&mut ctx, &body, &expected, Mode::Logical)?;

        self.fns.push(FnDecl {
            params: params.clone(),
            result: (**result).clone(),
            body: body.close_over(&vars),
        });
        Ok(FnId(self.fns.len() - 1))
    }

    pub(super) fn function(&self, id: FnId) -> Option<&FnDecl> {
        self.fns.get(id.0)
    }

    pub(super) fn struct_fields(&self, id: StructId) -> Option<&[Type]> {
        self.structs.get(id.0).map(|decl| decl.fields.as_slice())
    }
}
