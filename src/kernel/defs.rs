//! Global declarations. In K2 these are structs only.
//!
//! A declaration is checked against the declarations that precede it, so a
//! struct cannot mention itself, directly or through other declarations.

use std::rc::Rc;

use super::check::check_telescope;
use super::context::Context;
use super::error::KernelError;
use super::term::{StructId, Type};

#[derive(Clone, Debug)]
pub(super) struct StructDecl {
    /// A telescope, as in `Type::Tuple`.
    pub(super) fields: Vec<Type>,
}

#[derive(Clone, Debug, Default)]
pub struct Definitions {
    structs: Vec<StructDecl>,
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

    pub(super) fn struct_fields(&self, id: StructId) -> Option<&[Type]> {
        self.structs.get(id.0).map(|decl| decl.fields.as_slice())
    }
}
