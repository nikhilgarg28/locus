//! Physical Rust signatures are declarations in the execution IR only. They
//! never become kernel definitions or proofs, and always retain their effects.
use super::{Binder, FnRef, LowerError, Session};
use crate::{
    erased::{self, EBlock, EExpr, EFn},
    exec::{self, Promises},
    kernel::{Term, Type},
};
impl Session {
    pub(crate) fn declare_native(
        &mut self,
        name: &str,
        path: &str,
        params: &[Binder],
        result: &Type,
    ) -> Result<FnRef, LowerError> {
        let signature = Type::function_over(
            &params
                .iter()
                .map(|p| (p.id, p.ty.clone()))
                .collect::<Vec<_>>(),
            result,
        );
        let id = self.program.declare(exec::ExecFn {
            promises: Promises::default(),
            signature,
            params: params.iter().map(|p| p.id).collect(),
            body: exec::Block {
                stmts: vec![],
                tail: exec::Tail::Foreign {
                    path: path.into(),
                    arguments: params.iter().map(|p| Term::Free(p.id)).collect(),
                    result: result.clone(),
                },
            },
        })?;
        let reference = FnRef::Exec(id);
        let result = erased::erase_type(result);
        self.erased.fns.push(EFn {
            reference,
            name: name.into(),
            constant: false,
            params: params
                .iter()
                .map(|p| (p.id, p.name.clone(), erased::erase_type(&p.ty)))
                .collect(),
            passing: vec![],
            result: result.clone(),
            body: EBlock {
                stmts: vec![],
                tail: Some(Box::new(EExpr::NativeCall {
                    path: path.into(),
                    arguments: params
                        .iter()
                        .map(|p| EExpr::Var {
                            id: p.id,
                            name: p.name.clone(),
                        })
                        .collect(),
                    result,
                })),
            },
            owner: None,
            receiver: false,
        });
        erased::check_module(&self.erased)
            .map_err(|_| LowerError::Exec(exec::ExecError::InvalidForeign))?;
        Ok(reference)
    }
}
