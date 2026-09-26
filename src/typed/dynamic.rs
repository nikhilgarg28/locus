//! Checked adapters at the shared trait-object boundary.
use super::{Binder, ErasureLayout, FnRef, LowerError, Passing, Session};
use crate::{
    erased::*,
    exec,
    kernel::{StructId, Term, Type},
};

impl Session {
    pub(crate) fn declare_dynamic_interface(
        &mut self,
        name: String,
        methods: Vec<exec::DynMethod>,
    ) -> Result<StructId, LowerError> {
        let id = self
            .program
            .declare_dyn_interface(name.clone(), methods.clone())?;
        self.erased.dynamics.push(EDynInterface {
            id,
            name,
            methods: methods
                .iter()
                .map(|m| EDynMethod {
                    name: m.name.clone(),
                    params: m.params.iter().map(erase_type).collect(),
                    result: erase_type(&m.result),
                })
                .collect(),
        });
        Ok(id)
    }

    pub(crate) fn declare_dynamic_method(
        &mut self,
        name: String,
        interface: StructId,
        slot: usize,
        params: &[Binder],
        result: &Type,
    ) -> Result<FnRef, LowerError> {
        let id = self.program.declare(exec::ExecFn {
            promises: exec::Promises::default(),
            signature: Type::function_over(
                &params
                    .iter()
                    .map(|p| (p.id, p.ty.clone()))
                    .collect::<Vec<_>>(),
                result,
            ),
            params: params.iter().map(|p| p.id).collect(),
            body: exec::Block {
                stmts: vec![],
                tail: exec::Tail::DynCall {
                    interface,
                    slot,
                    receiver: Term::Free(params[0].id),
                    arguments: params[1..].iter().map(|p| Term::Free(p.id)).collect(),
                },
            },
        })?;
        let reference = FnRef::Exec(id);
        self.layouts.bindings.insert(
            params[0].id,
            ErasureLayout::Shared {
                lifetime: None,
                inner: Box::new(ErasureLayout::Default),
            },
        );
        let mut arguments = vec![EExpr::Var {
            id: params[0].id,
            name: params[0].name.clone(),
        }];
        arguments.extend(params[1..].iter().map(|p| EExpr::Var {
            id: p.id,
            name: p.name.clone(),
        }));
        self.erased.fns.push(EFn {
            reference,
            name,
            constant: false,
            params: params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        p.id,
                        p.name.clone(),
                        if i == 0 {
                            EType::Ref(None, Box::new(erase_type(&p.ty)))
                        } else {
                            erase_type(&p.ty)
                        },
                    )
                })
                .collect(),
            passing: vec![Passing::Value],
            result: erase_type(result),
            owner: None,
            receiver: false,
            body: EBlock {
                stmts: vec![],
                tail: Some(Box::new(EExpr::Dynamic {
                    operation: DynOperation::Call { interface, slot },
                    arguments,
                })),
            },
        });
        check_module(&self.erased).map_err(|_| {
            LowerError::Exec(exec::ExecError::InvalidDyn("erased dispatch adapter"))
        })?;
        Ok(reference)
    }

    pub(crate) fn declare_dynamic_pack(
        &mut self,
        name: String,
        interface: StructId,
        concrete: &Type,
        methods: Vec<exec::ExecFnId>,
        input: &Binder,
    ) -> Result<FnRef, LowerError> {
        let table = self.program.declare_dyn_table(exec::DynTable {
            interface,
            concrete: concrete.clone(),
            methods: methods.clone(),
        })?;
        self.erased.dyn_tables.push(EDynTable {
            id: table,
            interface,
            concrete: erase_type(concrete),
            methods: methods.into_iter().map(FnRef::Exec).collect(),
        });
        let result = Type::Struct(interface);
        let id = self.program.declare(exec::ExecFn {
            promises: exec::Promises::default(),
            signature: Type::Fn(vec![concrete.clone()], Box::new(result.clone())),
            params: vec![input.id],
            body: exec::Block {
                stmts: vec![],
                tail: exec::Tail::DynPack {
                    table,
                    value: Term::Free(input.id),
                },
            },
        })?;
        let reference = FnRef::Exec(id);
        let lifetime = Some("'locus".to_string());
        let layout = ErasureLayout::Shared {
            lifetime: lifetime.clone(),
            inner: Box::new(ErasureLayout::Default),
        };
        self.layouts.bindings.insert(input.id, layout.clone());
        self.layouts.functions.insert(reference, layout);
        self.erased.fns.push(EFn {
            reference,
            name,
            constant: false,
            params: vec![(
                input.id,
                input.name.clone(),
                EType::Ref(lifetime.clone(), Box::new(erase_type(concrete))),
            )],
            passing: vec![Passing::Value],
            result: EType::Ref(lifetime, Box::new(erase_type(&result))),
            owner: None,
            receiver: false,
            body: EBlock {
                stmts: vec![],
                tail: Some(Box::new(EExpr::Dynamic {
                    operation: DynOperation::Pack(table),
                    arguments: vec![EExpr::Var {
                        id: input.id,
                        name: input.name.clone(),
                    }],
                })),
            },
        });
        check_module(&self.erased).map_err(|_| {
            LowerError::Exec(exec::ExecError::InvalidDyn("erased coercion adapter"))
        })?;
        Ok(reference)
    }
}
