//! Physical collections share checked content snapshots, never borrow identity.
//! Every native operation is registered as a checked execution-IR helper.
use super::{
    calls::Argument,
    env::{Elab, Env, FnInfo},
    exprs::Value,
};
use crate::exec::BufferStorage;
use crate::{
    ast,
    kernel::{BufferOp, Term, Type, VarId},
    source::Span,
    typed::{self, ErasureLayout, Expr, Stmt},
};

impl Env<'_> {
    pub(super) fn collection_type(&mut self, element: &ast::Type, span: Span) -> Elab<Type> {
        self.require_preview(
            crate::preview::Feature::HeapViews,
            "physical collection",
            span,
        )?;
        let ty = self.ty(element)?;
        Ok(Type::Buffer(Box::new(ty)))
    }

    pub(super) fn array_length(&mut self, length: &ast::Expr) -> Elab<usize> {
        if let ast::ExprKind::Integer(n) = &length.kind
            && let Some(n) = n.value.to_u128().and_then(|n| usize::try_from(n).ok())
        {
            return Ok(n);
        }
        self.fail(
            "L0284",
            "an array length must be a non-negative integer literal",
            length.span,
        )
    }

    pub(super) fn native_buffer(
        &mut self,
        op: BufferOp,
        storage: BufferStorage,
        element: &Type,
        layout: ErasureLayout,
        arity: usize,
        span: Span,
    ) -> Elab<FnInfo> {
        self.native_buffer_mode(op, storage, element, layout, arity, span, false)
    }
    // Keep operation, physical shape and borrow mode explicit at the native boundary.
    #[allow(clippy::too_many_arguments)]
    fn native_buffer_mode(
        &mut self,
        op: BufferOp,
        storage: BufferStorage,
        element: &Type,
        layout: ErasureLayout,
        arity: usize,
        span: Span,
        borrowed: bool,
    ) -> Elab<FnInfo> {
        let found = self
            .session
            .buffer_functions()
            .iter()
            .find(|f| {
                f.borrowed == borrowed
                    && f.element_layout == layout
                    && f.op == op
                    && f.storage == storage
                    && &f.element == element
                    && (op != BufferOp::Literal || f.params.len() == arity)
            })
            .cloned();
        let helper = match found {
            Some(helper) => helper,
            None => {
                let mut serial = self.session.buffer_functions().len();
                let name = loop {
                    let name = format!("__locus_buffer_{serial}");
                    if !self.values.contains_key(&name)
                        && !self.session.erased().fns.iter().any(|f| f.name == name)
                    {
                        break name;
                    }
                    serial += 1;
                };
                let reason = "Rust array/slice/Vec operation preserves element order and implements the checked immutable buffer equation on normal return".to_string();
                let declared = if borrowed {
                    self.session.declare_buffer_borrow_function(
                        name,
                        storage,
                        element.clone(),
                        layout,
                        reason,
                    )
                } else {
                    self.session.declare_buffer_function(
                        name,
                        op,
                        storage,
                        element.clone(),
                        layout,
                        arity,
                        reason,
                    )
                };
                match declared {
                    Ok(helper) => helper,
                    Err(error) => return self.internal(error, span),
                }
            }
        };
        let result = if helper.exits.is_empty() {
            helper.result.clone()
        } else {
            let mut fields: Vec<_> = helper.exits.iter().map(|p| (p.id, p.ty.clone())).collect();
            fields.push((VarId::fresh(), helper.result.clone()));
            Type::tuple_over(&fields)
        };
        Ok(FnInfo {
            logical: false,
            result_logical: false,
            reference: helper.reference,
            name: helper.name,
            params: helper.params,
            result,
            constant: false,
            promises: helper.promises,
            passing: helper.passing,
            visibility: None,
            receiver: false,
        })
    }

    fn buffer_equation(&mut self, value: &Value, index: Option<usize>, span: Span) -> Elab<()> {
        let term = self.term(value, span)?;
        let proof_term = match index {
            Some(i) => Term::proj(term, i),
            None => term,
        };
        let proof = crate::kernel::Proof::OfTerm(proof_term);
        let inferred = crate::kernel::infer_proof(&mut self.ctx, &proof);
        let claim = self.kernel(inferred, span)?;
        self.facts.push(super::env::Fact::definition(proof, claim));
        Ok(())
    }

    fn collection_receiver(&mut self, receiver: &ast::Expr) -> Elab<Value> {
        self.suppress_models += 1;
        let value = self.ghost(|env| env.infer(receiver));
        self.suppress_models -= 1;
        value
    }

    fn collection_storage(
        &mut self,
        value: &Value,
        span: Span,
    ) -> Elab<(BufferStorage, ErasureLayout)> {
        let mut shape = self.session.expression_layout(&value.expr);
        while let ErasureLayout::Borrowed { inner, .. } | ErasureLayout::Shared { inner, .. } =
            shape
        {
            shape = *inner;
        }
        match shape {
            ErasureLayout::Buffer { storage, element } => Ok((storage, *element)),
            _ => self.fail(
                "L0284",
                "the collection's physical storage shape is unavailable",
                span,
            ),
        }
    }

    fn logical_buffer(
        &mut self,
        op: BufferOp,
        receiver: Value,
        index: Option<Value>,
        span: Span,
    ) -> Elab<Value> {
        let Type::Buffer(element) = &receiver.ty else {
            return self.fail("L0284", "expected a collection", span);
        };
        if op == BufferOp::Length {
            let source = self.term(&receiver, span)?;
            for upper in [false, true] {
                let proof = crate::kernel::Proof::BufferBound {
                    value: source.clone(),
                    upper,
                };
                let checked = crate::kernel::infer_proof(&mut self.ctx, &proof);
                let claim = self.kernel(checked, span)?;
                self.facts.push(super::env::Fact::definition(proof, claim));
            }
        }
        let mut arguments = vec![receiver.expr.clone()];
        let source_id = VarId::fresh();
        let mut parameters = vec![(source_id, receiver.ty.clone())];
        let mut model_args = vec![Term::Free(source_id)];
        if let Some(index) = index {
            let index_id = VarId::fresh();
            let source = self.term(&receiver, span)?;
            let actual_index = self.term(&index, span)?;
            let actual_index = if let Some(machine) = index.ty.as_machine() {
                Term::view(machine, actual_index)
            } else {
                actual_index
            };
            let model_index = if let Some(machine) = index.ty.as_machine() {
                Term::view(machine, Term::Free(index_id))
            } else {
                Term::Free(index_id)
            };
            parameters.push((index_id, index.ty));
            arguments.push(index.expr);
            model_args.push(model_index);
            for (i, claim) in crate::kernel::buffer::bounds(element, &source, &actual_index)
                .iter()
                .enumerate()
            {
                let evidence = self.solve(claim, span, None)?;
                let proof_id = VarId::fresh();
                let model_claim =
                    crate::kernel::buffer::bounds(element, &model_args[0], &model_args[1])[i]
                        .clone();
                parameters.push((proof_id, Type::proof(model_claim)));
                arguments.push(Expr::Proof(evidence));
                model_args.push(Term::proof(crate::kernel::Proof::OfTerm(Term::Free(
                    proof_id,
                ))));
            }
        }
        let ty = if op == BufferOp::Length {
            Type::Int
        } else {
            (**element).clone()
        };
        let body = Term::Buffer {
            op,
            element: (**element).clone(),
            arguments: model_args,
        };
        let callee = Term::lambda_over(&parameters, &ty, body);
        Ok(Value::new(
            Expr::LogicalApply {
                callee,
                arguments,
                ty: ty.clone(),
            },
            ty,
        ))
    }

    pub(super) fn vector_constructor(
        &mut self,
        callee: &ast::Expr,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Option<Elab<Value>> {
        let ast::ExprKind::Path(path) = &callee.kind else {
            return None;
        };
        let (owner, method) = path.pair()?;
        if owner.text != "Vec" || !matches!(method.text.as_str(), "new" | "from") {
            return None;
        }
        Some((|| {
            self.require_preview(crate::preview::Feature::HeapViews, "Vec construction", span)?;
            if self.total {
                return self.fail(
                    "L0284",
                    "a physical Vec cannot be constructed in logical computation",
                    span,
                );
            }
            let fields: &[ast::Expr] = match (method.text.as_str(), arguments) {
                ("new", []) => &[],
                (
                    "from",
                    [
                        ast::Expr {
                            kind: ast::ExprKind::Array(fields),
                            ..
                        },
                    ],
                ) => fields,
                _ => return self.fail("L0284", "use Vec::new() or Vec::from([values...])", span),
            };
            let mut element = match expected {
                Some(Type::Buffer(element)) => Some((**element).clone()),
                _ => None,
            };
            let element_hint = match self.layout_hints.get(&span) {
                Some(ErasureLayout::Buffer { element, .. }) => Some((**element).clone()),
                _ => None,
            };
            let mut values = Vec::new();
            for field in fields {
                if let Some(layout) = &element_hint {
                    self.expect_layout(field, layout);
                }
                let value = self.runtime_arguments(|env| match &element {
                    Some(ty) => env.check(field, ty),
                    None => env.infer(field),
                })?;
                element.get_or_insert_with(|| value.ty.clone());
                values.push((value, field.span));
            }
            let Some(element) = element else {
                return self.fail("L0284", "an empty Vec needs a Vec<T> type annotation", span);
            };
            let layout = match self.layout_hints.get(&span) {
                Some(ErasureLayout::Buffer { element, .. }) => (**element).clone(),
                _ => values
                    .first()
                    .map(|(value, _)| self.session.expression_layout(&value.expr))
                    .unwrap_or_default(),
            };
            let helper = self.native_buffer(
                BufferOp::Literal,
                BufferStorage::Vector,
                &element,
                layout,
                values.len(),
                span,
            )?;
            let args: Vec<_> = values
                .into_iter()
                .map(|(value, span)| Argument::Value(Box::new(value), span))
                .collect();
            let result = self.call_fn_with(&helper, &args, span)?;
            self.buffer_equation(&result, Some(1), span)?;
            self.field(result, 0, None, span)
        })())
    }

    pub(super) fn array_literal(
        &mut self,
        fields: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        self.require_preview(crate::preview::Feature::HeapViews, "array literal", span)?;
        if self.total {
            return self.fail(
                "L0284",
                "a physical array cannot be constructed in logical computation",
                span,
            );
        }
        let expected_element = match expected {
            Some(Type::Buffer(element)) => Some(&**element),
            _ => None,
        };
        let element_hint = match self.layout_hints.get(&span) {
            Some(ErasureLayout::Buffer { element, .. }) => Some((**element).clone()),
            _ => None,
        };
        let mut values = Vec::new();
        let mut element = expected_element.cloned();
        for field in fields {
            if let Some(layout) = &element_hint {
                self.expect_layout(field, layout);
            }
            let value = self.runtime_arguments(|env| match &element {
                Some(ty) => env.check(field, ty),
                None => env.infer(field),
            })?;
            element.get_or_insert_with(|| value.ty.clone());
            values.push((value, field.span));
        }
        let Some(element) = element else {
            return self.fail(
                "L0284",
                "an empty array needs an element type annotation",
                span,
            );
        };
        let element_layout = match self.layout_hints.get(&span) {
            Some(ErasureLayout::Buffer { element, .. }) => (**element).clone(),
            _ => values
                .first()
                .map(|(value, _)| self.session.expression_layout(&value.expr))
                .unwrap_or_default(),
        };
        if let Some(ErasureLayout::Buffer {
            storage: BufferStorage::Array(n),
            ..
        }) = self.layout_hints.get(&span)
            && *n != values.len()
        {
            return self.fail(
                "L0284",
                "array literal length does not match its declared type",
                span,
            );
        }
        let helper = self.native_buffer(
            BufferOp::Literal,
            BufferStorage::Array(values.len()),
            &element,
            element_layout,
            values.len(),
            span,
        )?;
        let arguments: Vec<_> = values
            .into_iter()
            .map(|(v, s)| Argument::Value(Box::new(v), s))
            .collect();
        let result = self.call_fn_with(&helper, &arguments, span)?;
        self.buffer_equation(&result, Some(1), span)?;
        self.field(result, 0, None, span)
    }

    pub(super) fn collection_method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Option<Elab<Value>> {
        // Only reserve these method names for a receiver already known to be
        // a buffer. Looking up a place avoids running or moving it twice.
        let is_buffer = super::mutation::place_path(receiver)
            .and_then(|(root, parts)| {
                let slot = self
                    .names
                    .iter()
                    .rposition(|local| local.name == root.text)?;
                self.place_steps(slot, &parts, super::mutation::Access::Lend)
                    .ok()
                    .map(|(_, ty)| matches!(ty, Type::Buffer(_)))
            })
            .unwrap_or(false);
        if !is_buffer && self.total && matches!(name.text.as_str(), "len" | "get") {
            // Logical receivers such as old!(items) are values rather than
            // physical places. Their observation retains no reference.
            let observed = self.collection_receiver(receiver);
            match observed {
                Ok(value) if matches!(value.ty, Type::Buffer(_)) => {
                    return Some((|| {
                        let op = if name.text == "len" {
                            BufferOp::Length
                        } else {
                            BufferOp::Get
                        };
                        if arguments.len() != usize::from(op == BufferOp::Get) {
                            return self.fail(
                                "L0208",
                                "wrong number of collection observation arguments",
                                span,
                            );
                        }
                        let index = if op == BufferOp::Get {
                            Some(self.infer(&arguments[0])?)
                        } else {
                            None
                        };
                        self.logical_buffer(op, value, index, span)
                    })());
                }
                Err(()) => return Some(Err(())),
                _ => {}
            }
        }
        if !is_buffer || !matches!(name.text.as_str(), "len" | "get" | "set" | "push") {
            return None;
        }
        Some((|| {
            let op = match name.text.as_str() {
                "len" => BufferOp::Length,
                "get" => BufferOp::Get,
                "set" => BufferOp::Set,
                _ => BufferOp::Push,
            };
            let expected = match op {
                BufferOp::Length => 0,
                BufferOp::Get | BufferOp::Push => 1,
                _ => 2,
            };
            if arguments.len() != expected {
                return self.fail(
                    "L0208",
                    format!("`{}` takes {expected} arguments", name.text),
                    span,
                );
            }
            self.buffer_operation(op, receiver, arguments, span)
        })())
    }

    pub(super) fn buffer_operation(
        &mut self,
        op: BufferOp,
        receiver: &ast::Expr,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        self.buffer_operation_mode(op, receiver, arguments, span, false)
    }
    pub(super) fn buffer_shared_index(
        &mut self,
        receiver: &ast::Expr,
        index: &ast::Expr,
        span: Span,
    ) -> Elab<Value> {
        self.buffer_operation_mode(
            BufferOp::Get,
            receiver,
            std::slice::from_ref(index),
            span,
            true,
        )
    }
    fn buffer_operation_mode(
        &mut self,
        op: BufferOp,
        receiver: &ast::Expr,
        arguments: &[ast::Expr],
        span: Span,
        borrowed: bool,
    ) -> Elab<Value> {
        self.require_preview(
            crate::preview::Feature::HeapViews,
            "collection operation",
            span,
        )?;
        let value = self.collection_receiver(receiver)?;
        let Type::Buffer(element) = &value.ty else {
            return self.fail(
                "L0284",
                "indexing requires an array, slice, or vector",
                receiver.span,
            );
        };
        let element = (**element).clone();
        let (storage, layout) = self.collection_storage(&value, span)?;
        if self.total {
            if !matches!(op, BufferOp::Length | BufferOp::Get) {
                return self.fail(
                    "L0284",
                    "logical observations cannot mutate collections",
                    span,
                );
            }
            let index = if op == BufferOp::Get {
                Some(self.infer(&arguments[0])?)
            } else {
                None
            };
            return self.logical_buffer(op, value, index, span);
        }
        if op == BufferOp::Get
            && !borrowed
            && !layout.is_logical()
            && !self
                .session
                .program()
                .definitions()
                .is_erased_type(&element)
            && !self.is_copy(&element)
        {
            return self.fail(
                "L0284",
                "indexing cannot move a non-Copy element out of borrowed storage",
                span,
            );
        }
        let helper = self.native_buffer_mode(op, storage, &element, layout, 0, span, borrowed)?;
        let borrowed = ast::Expr {
            span: receiver.span,
            kind: ast::ExprKind::Ref {
                mutable: matches!(op, BufferOp::Set | BufferOp::Push),
                expr: Box::new(receiver.clone()),
            },
        };
        let mut written = vec![borrowed];
        written.extend_from_slice(arguments);
        if matches!(op, BufferOp::Get | BufferOp::Set) {
            for _ in 0..2 {
                written.push(ast::Expr {
                    span,
                    kind: ast::ExprKind::Hole,
                });
            }
        }
        let expected_layout = self.layout_hints.remove(&span);
        let called = self.call_fn(&helper, &written, span);
        self.layout_hints.remove(&span);
        if let Some(layout) = expected_layout {
            self.layout_hints.insert(span, layout);
        }
        let result = called?;
        self.buffer_equation(
            &result,
            if op == BufferOp::Set { None } else { Some(1) },
            span,
        )?;
        if matches!(op, BufferOp::Length | BufferOp::Get) {
            self.field(result, 0, None, span)
        } else {
            Ok(result)
        }
    }

    pub(super) fn collection_assignment(
        &mut self,
        receiver: &ast::Expr,
        index: &ast::Expr,
        value: &ast::Expr,
        span: Span,
    ) -> Elab<Stmt> {
        // Preserve Rust assignment evaluation order: RHS precedes the place.
        let rhs = self.infer(value)?;
        let term = self.term(&rhs, value.span)?;
        let name = ast::Name {
            text: format!("__locus_assignment_{}", self.names.len()),
            span: value.span,
        };
        let pattern = ast::Pattern {
            span: value.span,
            kind: ast::PatternKind::Name {
                name: name.clone(),
                mutable: false,
            },
        };
        let binding = self.bind_pattern(&pattern, term, false)?;
        self.register_pattern_layout(&binding, &self.session.expression_layout(&rhs.expr));
        let stored = ast::Expr {
            span: value.span,
            kind: ast::ExprKind::Name(name),
        };
        let result =
            self.buffer_operation(BufferOp::Set, receiver, &[index.clone(), stored], span)?;
        Ok(Stmt::Expr(Expr::Block(typed::Block {
            stmts: vec![
                Stmt::Let {
                    pattern: binding,
                    value: rhs.expr,
                },
                Stmt::Expr(result.expr),
            ],
            tail: None,
        })))
    }
}
