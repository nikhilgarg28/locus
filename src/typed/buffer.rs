//! Checked native collection helpers. Call sites use ordinary CallFn/Lend,
//! so native operations share the same SSA writeback and proof invalidation.
use super::{Binder, ErasureLayout, FnRef, LowerError, Passing, Session};
use crate::erased::{self, EBlock, EExpr, EFn, EPattern, EPlace, EStmt};
use crate::exec::{self, BufferStmt, BufferStorage, ExecFn, Lending, Promises};
use crate::kernel::buffer::{bounds, length};
use crate::kernel::{BufferOp, HypId, Proof, Term, Type, VarId};

#[derive(Clone, Debug)]
pub struct BufferFunction {
    pub reference: FnRef,
    pub borrowed: bool,
    pub name: String,
    pub op: BufferOp,
    pub storage: BufferStorage,
    pub element: Type,
    pub element_layout: ErasureLayout,
    pub params: Vec<Binder>,
    pub passing: Vec<Passing>,
    pub result: Type,
    pub exits: Vec<Binder>,
    pub result_layout: ErasureLayout,
    pub promises: Promises,
    /// Included in audit output: the runtime standard-library assumption.
    pub reason: String,
}

fn binder(name: impl Into<String>, ty: Type) -> Binder {
    Binder {
        id: VarId::fresh(),
        name: name.into(),
        ty,
        ghost: false,
    }
}
fn proof_value(id: VarId) -> Term {
    Term::proof(Proof::OfTerm(Term::Free(id)))
}
fn buffer_type(element: &Type) -> Type {
    Type::Buffer(Box::new(element.clone()))
}

impl Session {
    pub fn buffer_functions(&self) -> &[BufferFunction] {
        &self.buffers
    }

    /// Register a private runtime helper only after its IR contract checks.
    /// The reason is mandatory even for standard-library-backed operations.
    // The checked native signature keeps storage, logical type and layout explicit.
    #[allow(clippy::too_many_arguments)]
    pub fn declare_buffer_function(
        &mut self,
        name: String,
        op: BufferOp,
        storage: BufferStorage,
        element: Type,
        element_layout: ErasureLayout,
        literal_arity: usize,
        reason: String,
    ) -> Result<BufferFunction, LowerError> {
        self.declare_buffer_mode(
            name,
            op,
            storage,
            element,
            element_layout,
            literal_arity,
            reason,
            false,
        )
    }
    /// Checked indexed observation, returned as a shared reference into the
    /// input storage. The kernel value contract is exactly Get; permissions
    /// additionally tie the returned physical reference to the input lifetime.
    pub fn declare_buffer_borrow_function(
        &mut self,
        name: String,
        storage: BufferStorage,
        element: Type,
        element_layout: ErasureLayout,
        reason: String,
    ) -> Result<BufferFunction, LowerError> {
        self.declare_buffer_mode(
            name,
            BufferOp::Get,
            storage,
            element,
            element_layout,
            0,
            reason,
            true,
        )
    }
    // This is the common IR builder for value and borrowed native operations.
    #[allow(clippy::too_many_arguments)]
    fn declare_buffer_mode(
        &mut self,
        name: String,
        op: BufferOp,
        storage: BufferStorage,
        element: Type,
        element_layout: ErasureLayout,
        literal_arity: usize,
        reason: String,
        borrowed: bool,
    ) -> Result<BufferFunction, LowerError> {
        if reason.trim().is_empty() {
            return Err(
                exec::ExecError::InvalidBuffer("a native operation needs an audit reason").into(),
            );
        }
        if self.erased.fns.iter().any(|f| f.name == name) {
            return Err(exec::ExecError::InvalidBuffer("duplicate native helper name").into());
        }
        let layout = ErasureLayout::Buffer {
            storage,
            element: Box::new(element_layout.clone()),
        };
        let allocating = op == BufferOp::Push
            || (op == BufferOp::Literal && storage == BufferStorage::Vector && literal_arity > 0);
        let promises = Promises {
            terminates: true,
            no_panic: !allocating,
            no_alloc: !allocating,
            no_io: true,
        };
        let mut params = Vec::new();
        let mut passing = Vec::new();
        let mut parameter_layouts = Vec::new();
        if op == BufferOp::Literal {
            for index in 0..literal_arity {
                params.push(binder(format!("item{index}"), element.clone()));
                passing.push(Passing::Value);
                parameter_layouts.push(element_layout.clone());
            }
        } else {
            params.push(binder("items", buffer_type(&element)));
            passing.push(if matches!(op, BufferOp::Set | BufferOp::Push) {
                Passing::RefMut
            } else {
                Passing::Ref
            });
            parameter_layouts.push(if borrowed {
                ErasureLayout::Borrowed {
                    lifetime: Some("'a".into()),
                    inner: Box::new(layout.clone()),
                }
            } else {
                layout.clone()
            });
            if matches!(op, BufferOp::Get | BufferOp::Set) {
                params.push(binder(
                    "index",
                    Type::machine(self.program.pointer_width().usize()),
                ));
                passing.push(Passing::Value);
                parameter_layouts.push(ErasureLayout::Default);
            }
            if matches!(op, BufferOp::Set | BufferOp::Push) {
                params.push(binder("value", element.clone()));
                passing.push(Passing::Value);
                parameter_layouts.push(element_layout.clone());
            }
        }
        for (parameter, layout) in params.iter_mut().zip(&parameter_layouts) {
            parameter.ghost = layout.is_logical();
        }
        let data_count = params.len();
        let arguments: Vec<_> = params
            .iter()
            .map(|p| {
                if matches!(p.ty, Type::Proof(_)) {
                    proof_value(p.id)
                } else {
                    Term::Free(p.id)
                }
            })
            .collect();
        let mut evidence = Vec::new();
        if matches!(op, BufferOp::Get | BufferOp::Set) {
            for (index, claim) in bounds(
                &element,
                &arguments[0],
                &Term::view(self.program.pointer_width().usize(), arguments[1].clone()),
            )
            .into_iter()
            .enumerate()
            {
                let p = binder(format!("bound{index}"), Type::proof(claim));
                evidence.push(Proof::OfTerm(Term::Free(p.id)));
                params.push(p);
                passing.push(Passing::Value);
                parameter_layouts.push(ErasureLayout::Logical);
            }
        }
        let value_type = match op {
            BufferOp::Length => Type::machine(self.program.pointer_width().usize()),
            BufferOp::Get => element.clone(),
            _ => buffer_type(&element),
        };
        let value_layout = match op {
            BufferOp::Length => ErasureLayout::Default,
            BufferOp::Get if borrowed => ErasureLayout::Shared {
                lifetime: Some("'a".into()),
                inner: Box::new(element_layout.clone()),
            },
            BufferOp::Get => element_layout.clone(),
            _ => layout.clone(),
        };
        let out = binder("out", value_type.clone());
        let equation = HypId::fresh();
        let room_hyp = HypId::fresh();
        let mut model_args = arguments.clone();
        if matches!(op, BufferOp::Get | BufferOp::Set) {
            model_args = vec![
                arguments[0].clone(),
                Term::view(self.program.pointer_width().usize(), arguments[1].clone()),
            ];
            model_args.extend(evidence.iter().cloned().map(Term::proof));
            if op == BufferOp::Set {
                model_args.push(arguments[2].clone());
            }
        }
        let mut result_fields = Vec::<Binder>::new();
        let mut result_values = Vec::<Term>::new();
        let mut result_layouts = Vec::new();
        let mut exits = Vec::new();
        if matches!(op, BufferOp::Set | BufferOp::Push) {
            exits.push(out.clone());
        } else {
            result_fields.push(out.clone());
            result_values.push(if matches!(out.ty, Type::Proof(_)) {
                proof_value(out.id)
            } else {
                Term::Free(out.id)
            });
            result_layouts.push(value_layout.clone());
        }
        if op == BufferOp::Push {
            let room = binder(
                "room",
                Type::proof(Term::int_lt(
                    length(element.clone(), arguments[0].clone()),
                    Term::Int(self.program.pointer_width().usize().max()),
                )),
            );
            model_args.push(proof_value(room.id));
            result_values.push(Term::proof(Proof::hyp(room_hyp)));
            result_layouts.push(ErasureLayout::Logical);
            result_fields.push(room);
        }
        let model = Term::Buffer {
            op,
            element: element.clone(),
            arguments: model_args,
        };
        let actual = if op == BufferOp::Length {
            Term::view(self.program.pointer_width().usize(), Term::Free(out.id))
        } else {
            Term::Free(out.id)
        };
        let eq_type = if op == BufferOp::Length {
            Type::Int
        } else {
            value_type.clone()
        };
        let (established, returned_evidence) = if let Type::Proof(claim) = &eq_type {
            // Reading stored evidence establishes its proposition directly.
            // Equality between proof objects is intentionally not a kernel type.
            (
                binder("established", Type::proof((**claim).clone())),
                proof_value(out.id),
            )
        } else {
            (
                binder("established", Type::proof(Term::eq(eq_type, actual, model))),
                Term::proof(Proof::hyp(equation)),
            )
        };
        result_fields.push(established);
        result_values.push(returned_evidence);
        result_layouts.push(ErasureLayout::Logical);
        let result = if op == BufferOp::Set {
            result_fields[0].ty.clone()
        } else {
            Type::tuple_over(
                &result_fields
                    .iter()
                    .map(|f| (f.id, f.ty.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        let result_layout = if op == BufferOp::Set {
            ErasureLayout::Logical
        } else {
            ErasureLayout::Tuple(result_layouts)
        };
        let result_term = if op == BufferOp::Set {
            result_values.remove(0)
        } else {
            let Type::Tuple(fields) = &result else {
                unreachable!()
            };
            Term::Tuple(fields.clone(), result_values)
        };
        let (exec_result, exec_tail) = if exits.is_empty() {
            (result.clone(), result_term)
        } else {
            let ty =
                Type::tuple_over(&[(out.id, out.ty.clone()), (VarId::fresh(), result.clone())]);
            let Type::Tuple(fields) = &ty else {
                unreachable!()
            };
            let term = Term::Tuple(fields.clone(), vec![Term::Free(out.id), result_term]);
            (ty, term)
        };
        let signature = Type::function_over(
            &params
                .iter()
                .map(|p| (p.id, p.ty.clone()))
                .collect::<Vec<_>>(),
            &exec_result,
        );
        let operation = BufferStmt {
            var: out.id,
            equation,
            op,
            storage,
            element: element.clone(),
            logical_payload: element_layout.is_logical()
                || self.program.definitions().is_erased_type(&element),
            arguments,
            bounds: evidence,
            learned: if op == BufferOp::Push {
                vec![room_hyp]
            } else {
                vec![]
            },
        };
        let reference = FnRef::Exec(self.program.declare(ExecFn {
            promises,
            signature,
            params: params.iter().map(|p| p.id).collect(),
            body: exec::Block {
                stmts: vec![exec::Stmt::Buffer(Box::new(operation))],
                tail: exec::Tail::Value(exec_tail),
            },
        })?);
        for (param, layout) in params.iter().zip(&parameter_layouts) {
            self.layouts.bindings.insert(param.id, layout.clone());
        }
        self.layouts
            .functions
            .insert(reference, result_layout.clone());
        let erase = |ty: &Type, layout: &ErasureLayout| {
            erased::type_with_layout(ty, layout, &erased::erase_type)
        };
        let erased_params = params
            .iter()
            .zip(&parameter_layouts)
            .map(|(p, l)| (p.id, p.name.clone(), erase(&p.ty, l)))
            .collect();
        let native_args = params[..data_count]
            .iter()
            .enumerate()
            .map(|(index, p)| {
                if index == 0 && op != BufferOp::Literal {
                    EExpr::Lend {
                        mutable: passing[0] == Passing::RefMut,
                        place: EPlace {
                            id: p.id,
                            name: p.name.clone(),
                            path: vec![],
                        },
                    }
                } else {
                    EExpr::Var {
                        id: p.id,
                        name: p.name.clone(),
                    }
                }
            })
            .collect();
        let native = EExpr::Buffer {
            op,
            storage,
            element: erase(&element, &element_layout),
            arguments: native_args,
        };
        let native = if borrowed {
            EExpr::Shared {
                value: Box::new(native),
                lifetime: Some("'a".into()),
            }
        } else {
            native
        };
        let erased_body = if matches!(op, BufferOp::Set | BufferOp::Push) {
            EBlock {
                stmts: vec![EStmt::Expr(native)],
                tail: Some(Box::new(if op == BufferOp::Set {
                    EExpr::Proved
                } else {
                    EExpr::Tuple(vec![EExpr::Proved, EExpr::Proved])
                })),
            }
        } else if op == BufferOp::Get
            && !borrowed
            && erase(&out.ty, &value_layout) == erased::EType::Ghost
        {
            EBlock {
                stmts: vec![EStmt::Let {
                    pattern: EPattern::Wildcard,
                    value: native,
                }],
                tail: Some(Box::new(EExpr::Tuple(vec![EExpr::Ghost, EExpr::Proved]))),
            }
        } else {
            EBlock {
                stmts: vec![EStmt::Let {
                    pattern: EPattern::Bind {
                        id: out.id,
                        name: out.name.clone(),
                        ty: erase(&out.ty, &value_layout),
                        mutable: false,
                    },
                    value: native,
                }],
                tail: Some(Box::new(EExpr::Tuple(vec![
                    EExpr::Var {
                        id: out.id,
                        name: out.name.clone(),
                    },
                    EExpr::Proved,
                ]))),
            }
        };
        self.erased.fns.push(EFn {
            reference,
            name: name.clone(),
            constant: false,
            params: erased_params,
            passing: passing.clone(),
            result: erase(&result, &result_layout),
            body: erased_body,
            owner: None,
            receiver: false,
        });
        if let FnRef::Exec(id) = reference {
            self.lending.insert(
                id,
                Lending {
                    params: if exits.is_empty() {
                        vec![]
                    } else {
                        vec![(0, vec![params[0].id, out.id])]
                    },
                    calls: HashMap::new(),
                    projections: HashMap::from([(
                        out.id,
                        exec::projection::ValueProjection::new(
                            erase(&out.ty, &value_layout),
                            &self.erased,
                        ),
                    )]),
                },
            );
        }
        let function = BufferFunction {
            reference,
            borrowed,
            name,
            op,
            storage,
            element,
            element_layout,
            params,
            passing,
            result,
            exits,
            result_layout,
            promises,
            reason,
        };
        self.buffers.push(function.clone());
        Ok(function)
    }
}
use std::collections::HashMap;
