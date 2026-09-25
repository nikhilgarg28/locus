//! Checked observational models. A model is an ordinary logical definition
//! whose one source argument is borrowed for the observation. Registration
//! adds no facts: its defining equation is checked by the same kernel.
use super::calls::Argument;
use super::env::{Elab, Env, FnInfo};
use super::exprs::Value;
use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::kernel::{MachineInt, Type};
use crate::source::Span;
use crate::typed::{ErasureLayout, Expr, Passing};
use std::rc::Rc;

#[derive(Clone)]
pub(super) enum Implementation {
    Identity,
    IntegerView,
    NaturalView,
    BooleanView,
    Definition(Rc<FnInfo>),
    Structural(Rc<super::env::StructInfo>, Vec<ModelEntry>),
}
#[derive(Clone)]
pub(super) struct ModelEntry {
    pub source: Type,
    pub target: Type,
    pub source_layout: ErasureLayout,
    pub implementation: Implementation,
    pub span: Option<Span>,
    pub default: bool,
}

pub(super) fn primitive_models(natural: Type) -> Vec<ModelEntry> {
    let mut models: Vec<_> = MachineInt::ALL
        .into_iter()
        .map(|machine| ModelEntry {
            source: Type::machine(machine),
            target: if machine.signed() {
                Type::Int
            } else {
                natural.clone()
            },
            source_layout: ErasureLayout::Default,
            implementation: if machine.signed() {
                Implementation::IntegerView
            } else {
                Implementation::NaturalView
            },
            span: None,
            default: true,
        })
        .collect();
    models.push(ModelEntry {
        source: Type::Bool,
        target: Type::Bool,
        source_layout: ErasureLayout::Default,
        implementation: Implementation::BooleanView,
        span: None,
        default: true,
    });
    models
}

impl Env<'_> {
    pub(super) fn derive_model(
        &mut self,
        info: &Rc<super::env::StructInfo>,
        span: Span,
    ) -> Elab<()> {
        let source = Type::Struct(info.id);
        if self.session.program().definitions().is_erased_type(&source) {
            return self.fail("L0282", "a Logical type already is its own logical value; derive Model on a physical struct", span);
        }
        let name = format!("{}Model", info.name);
        if self.types.contains_key(&name) {
            return self.fail(
                "L0282",
                format!("derived model name `{name}` is already declared"),
                span,
            );
        }
        let mut entries = Vec::new();
        let mut fields = Vec::new();
        for field in &info.fields {
            let layout = self.session.binding_layout(field.id);
            let entry = if self
                .session
                .program()
                .definitions()
                .is_erased_type(&field.ty)
                || field.ghost
                || layout == ErasureLayout::Logical
            {
                ModelEntry {
                    source: field.ty.clone(),
                    target: field.ty.clone(),
                    source_layout: layout.clone(),
                    implementation: Implementation::Identity,
                    span: Some(span),
                    default: true,
                }
            } else if let Some(entry) = self.models.iter().find(|entry| {
                entry.source == field.ty
                    && super::layout::borrow_compatible(&field.ty, &layout, &entry.source_layout)
            }) {
                entry.clone()
            } else {
                return self.fail("L0282", format!("cannot derive Model: field `{}.{}` has no canonical model; write a model that selects the relevant fields", info.name, field.name), span);
            };
            fields.push(crate::typed::Binder {
                id: crate::kernel::VarId::fresh(),
                name: field.name.clone(),
                ty: entry.target.clone(),
                ghost: true,
            });
            entries.push(entry);
        }
        let item = crate::typed::StructItem {
            name: name.clone(),
            fields: fields.clone(),
            derives: vec![],
        };
        let id = match self.session.declare_struct(&item) {
            Ok(id) => id,
            Err(_) => return self.fail("L0282", "cannot derive a model with dependent proof fields; define its logical representation explicitly", span),
        };
        if let Err(error) = self.session.mark_logical_type(&Type::Struct(id)) {
            return self.internal(error, span);
        }
        let derived = Rc::new(super::env::StructInfo {
            captures: Vec::new(),
            origin: info.origin,
            id,
            name: name.clone(),
            fields,
            derives: vec![],
            visibility: info.visibility.clone(),
            field_visibility: info.field_visibility.clone(),
        });
        self.types
            .insert(name, super::env::Global::Struct(derived.clone()));
        self.models.push(ModelEntry {
            source,
            target: Type::Struct(id),
            source_layout: ErasureLayout::Default,
            implementation: Implementation::Structural(derived, entries),
            span: Some(span),
            default: true,
        });
        Ok(())
    }
    /// Explicit observation: resolve only a physical read path, then apply
    /// the selected value's canonical model. The read still checks ownership.
    pub(super) fn observe_place(&mut self, arguments: &[ast::Expr], span: Span) -> Elab<Value> {
        let [place] = arguments else {
            return self.fail("L0282", "model! takes exactly one physical read path", span);
        };
        fn is_place(expr: &ast::Expr) -> bool {
            match &expr.kind {
                ast::ExprKind::Name(_) | ast::ExprKind::Path(_) => true,
                ast::ExprKind::Group(inner) => is_place(inner),
                ast::ExprKind::Form {
                    form: ast::Form::Old,
                    arguments,
                    ..
                } => matches!(arguments.as_slice(), [inner] if is_place(inner)),
                ast::ExprKind::Member { value, .. } | ast::ExprKind::Index { value, .. } => {
                    is_place(value)
                }
                ast::ExprKind::Unary {
                    operator: ast::UnaryOp::Deref,
                    expr,
                    ..
                } => is_place(expr),
                ast::ExprKind::Subscript { value, .. } => is_place(value),
                _ => false,
            }
        }
        if !is_place(place) {
            return self.fail("L0282", "model! observes a physical read path; bind runtime computations before observing them", place.span);
        }
        let diagnostics = self.diagnostics.len();
        self.suppress_models += 1;
        self.explicit_model_depth += 1;
        let value = self.logical("a model observation", |env| {
            env.ghost(|env| env.infer(place))
        });
        self.explicit_model_depth -= 1;
        self.suppress_models -= 1;
        let value = value?;
        if self
            .diagnostics
            .iter()
            .skip(diagnostics)
            .any(Diagnostic::is_error)
        {
            return Err(());
        }
        let source = self.show_type(&value.ty);
        let observed = self.default_model(value, place.span)?;
        if !(self
            .session
            .program()
            .definitions()
            .is_erased_type(&observed.ty)
            || observed.ty == Type::Bool && super::reconcile::is_logical_expr(&observed.expr))
        {
            return self.fail(
                "L0282",
                format!("`{source}` has no canonical logical model"),
                place.span,
            );
        }
        Ok(observed)
    }

    pub(super) fn model_shape(
        &mut self,
        model: &ast::ModelImpl,
        methods: &[ast::Declaration],
    ) -> Elab<()> {
        self.require_preview(
            crate::preview::Feature::LogicalData,
            "Model implementation",
            model.span,
        )?;
        let [method] = methods else {
            return self.fail(
                "L0282",
                "a Model implementation contains exactly one `logic fn model(source: &T) -> Self::Logic`",
                model.span,
            );
        };
        let ast::DeclarationKind::Function {
            logical: true,
            name,
            self_param: None,
            generics,
            parameters,
            ..
        } = &method.kind
        else {
            return self.fail(
                "L0282",
                "a Model implementation must define `logic fn model(source: &T) -> Self::Logic`",
                method.span,
            );
        };
        if self.text(name.span) != "model"
            || !generics.is_empty()
            || parameters.len() != 1
            || parameters[0].mutable
            || matches!(
                parameters[0].ty.observed().kind,
                ast::TypeKind::Ref { mutable: true, .. }
            )
        {
            return self.fail("L0282", "the Model method takes one read-only source observation: `logic fn model(source: T) -> Self::Logic`", method.span);
        }
        Ok(())
    }

    pub(super) fn register_model(
        &mut self,
        model: &ast::ModelImpl,
        mut info: Rc<FnInfo>,
    ) -> Elab<()> {
        let source = match &model.source.kind {
            ast::TypeKind::Slice(element) => self.collection_type(element, model.source.span)?,
            _ => self.ty(&model.source)?,
        };
        if self.session.program().definitions().is_erased_type(&source) {
            return self.fail(
                "L0282",
                "Model observes a physical type; Logical types are already logical values",
                model.source.span,
            );
        }
        let source_layout = self.written_layout(&model.source);
        let target = self.ty(&model.target)?;
        if !self.logical_spelling(&model.target) {
            return self.fail(
                "L0282",
                "the destination of Model must be Logical",
                model.target.span,
            );
        }
        if !info.logical
            || info.params.len() != 1
            || info.passing != [Passing::Value]
            || info.params[0].ty != source
            || info.result != target
        {
            return self.fail("L0282", "the Model method must observe the declared source and return the declared logical destination", model.span);
        }
        let mut parameter_layout = self.session.binding_layout(info.params[0].id);
        while let ErasureLayout::Shared { inner, .. } | ErasureLayout::Borrowed { inner, .. } =
            parameter_layout
        {
            parameter_layout = *inner;
        }
        if parameter_layout != source_layout {
            return self.fail(
                "L0282",
                "the Model method's source shape must match its declared source type exactly",
                model.span,
            );
        }
        if let Some(earlier) = self.models.iter().find(|entry| {
            entry.source == source
                && (super::layout::borrow_compatible(&source, &source_layout, &entry.source_layout)
                    || super::layout::borrow_compatible(
                        &source,
                        &entry.source_layout,
                        &source_layout,
                    ))
        }) {
            let mut diagnostic = Diagnostic::error(
                "L0282",
                "this physical type already has a canonical Model implementation",
                model.span,
            );
            if let Some(span) = earlier.span {
                diagnostic = diagnostic.label(span, "first implementation");
            } else {
                diagnostic =
                    diagnostic.note("the source has a primitive model supplied by the compiler");
            }
            self.diagnostics.push(diagnostic);
            return Err(());
        }
        // The canonical observation is available wherever its source can be
        // read. Only this registry entry bypasses ordinary method visibility;
        // the checked body still obeys the implementation module's privacy.
        Rc::make_mut(&mut info).origin = None;
        self.models.push(ModelEntry {
            source,
            target,
            source_layout,
            implementation: Implementation::Definition(info),
            span: Some(model.span),
            default: true,
        });
        Ok(())
    }

    /// Called after resolving the logical destination. Read the source under
    /// the same availability/permission checks as erased shared arguments.
    /// Eager runtime argument work remains in the resulting typed expression.
    fn model_source(
        &mut self,
        inner: &ast::Expr,
        target: &Type,
        span: Span,
    ) -> Elab<(ModelEntry, Value)> {
        self.suppress_models += 1;
        self.explicit_model_depth += 1;
        let value = self.ghost(|env| env.infer(inner));
        self.explicit_model_depth -= 1;
        self.suppress_models -= 1;
        let value = value?;
        if &value.ty == target && self.session.program().definitions().is_erased_type(target) {
            return Ok((
                ModelEntry {
                    source: value.ty.clone(),
                    target: target.clone(),
                    source_layout: ErasureLayout::Logical,
                    implementation: Implementation::Identity,
                    span: None,
                    default: false,
                },
                value,
            ));
        }
        let mut actual_layout = self.session.expression_layout(&value.expr);
        while let ErasureLayout::Shared { inner, .. } | ErasureLayout::Borrowed { inner, .. } =
            actual_layout
        {
            actual_layout = *inner;
        }
        let entry = self
            .models
            .iter()
            .find(|entry| {
                entry.source == value.ty
                    && &entry.target == target
                    && super::layout::borrow_compatible(
                        &value.ty,
                        &actual_layout,
                        &entry.source_layout,
                    )
            })
            .cloned();
        let Some(entry) = entry else {
            let source = self.show_type(&value.ty);
            let target = self.show_type(target);
            return self.fail(
                "L0282",
                format!("no Model implementation observes `{source}` as `{target}`"),
                span,
            );
        };
        Ok((entry, value))
    }

    pub(super) fn model_cast(
        &mut self,
        inner: &ast::Expr,
        target: &Type,
        span: Span,
    ) -> Elab<Value> {
        // A cast to a logical type elaborates its operand in the logic,
        // which observes physical names through their one canonical model.
        // It cannot hide executable calls; bind those before observing them.
        let previous = std::mem::replace(&mut self.suppress_models, 0);
        let value = self.logical("a logical cast", |env| env.infer(inner));
        self.suppress_models = previous;
        let value = value?;
        if &value.ty == target {
            return Ok(value);
        }
        if self.is_natural(&value.ty) && target == &Type::Int {
            return self.natural_integer(value, span);
        }
        let from = self.show_type(&value.ty);
        let to = self.show_type(target);
        self.fail("L0282", format!("no logical conversion from `{from}` to `{to}`; use the canonical model or an explicitly checked conversion"), span)
    }

    pub(super) fn observation_definition_selector(
        &mut self,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<(crate::kernel::FnId, crate::kernel::Term)> {
        self.logical("a model definition selector", |env| {
            let observed = env.observe_place(arguments, span)?;
            let call = env.term(&observed, span)?;
            let mut expr = &observed.expr;
            while let Expr::Ghost(inner) = expr { expr = inner; }
            let Expr::CallMath { id, .. } = expr else {
                return env.fail("L0229", "a primitive or derived model has no source definition to fold; use its checked model laws", span);
            };
            Ok((*id, call))
        })
    }

    pub(super) fn model_definition_selector(
        &mut self,
        inner: &ast::Expr,
        target: &ast::Type,
        span: Span,
    ) -> Elab<(crate::kernel::FnId, crate::kernel::Term)> {
        self.logical("a model definition selector", |env| {
            let target = env.ty(target)?;
            let (entry, value) = env.model_source(inner, &target, span)?;
            let Implementation::Definition(info) = &entry.implementation else {
                return env.fail("L0229", "a primitive model has no source definition to fold; use its checked model laws", span);
            };
            let crate::typed::FnRef::Math(id) = info.reference else { return env.fail("L0229", "a model selector requires a logical definition", span); };
            if !crate::typed::is_pure(&value.expr) {
                return env.fail("L0229", "a model definition selector cannot have runtime effects", inner.span);
            }
            let value = env.apply_model(&entry, value, span)?;
            let call = env.term(&value, span)?;
            Ok((id, call))
        })
    }

    pub(super) fn default_model(&mut self, value: Value, span: Span) -> Elab<Value> {
        let entry = self
            .models
            .iter()
            .find(|entry| {
                entry.default && entry.source == value.ty && {
                    let mut layout = self.session.expression_layout(&value.expr);
                    while let ErasureLayout::Shared { inner, .. }
                    | ErasureLayout::Borrowed { inner, .. } = layout
                    {
                        layout = *inner;
                    }
                    super::layout::borrow_compatible(&value.ty, &layout, &entry.source_layout)
                }
            })
            .cloned();
        match entry {
            Some(entry) => self.apply_model(&entry, value, span),
            None => Ok(value),
        }
    }

    fn apply_model(&mut self, entry: &ModelEntry, value: Value, span: Span) -> Elab<Value> {
        // Modeling can replace a physical place with a logical construction.
        // Check its availability before that replacement loses the place path.
        let diagnostics = self.diagnostics.len();
        self.place_taken(&value, span);
        if self
            .diagnostics
            .iter()
            .skip(diagnostics)
            .any(Diagnostic::is_error)
        {
            return Err(());
        }
        match &entry.implementation {
            Implementation::Identity => Ok(value),
            Implementation::NaturalView => {
                let machine = value.ty.as_machine().expect("primitive unsigned model");
                let term = self.term(&value, span)?;
                let proof =
                    crate::kernel::Proof::Axiom(crate::kernel::Axiom::ViewLower(machine, term));
                let integer = Value::new(
                    Expr::Cast {
                        expr: Box::new(value.expr),
                        from: value.ty,
                        to: Type::Int,
                    },
                    Type::Int,
                );
                self.make_natural(integer, Some(proof), span)
            }
            Implementation::IntegerView => Ok(Value::new(
                Expr::Cast {
                    expr: Box::new(value.expr),
                    from: value.ty,
                    to: Type::Int,
                },
                Type::Int,
            )),
            Implementation::BooleanView => {
                if super::reconcile::is_logical_expr(&value.expr) {
                    return Ok(value);
                }
                Ok(Value::new(Expr::Ghost(Box::new(value.expr)), Type::Bool))
            }
            Implementation::Structural(info, entries) => {
                let mut fields = Vec::new();
                for (index, (field, entry)) in info.fields.iter().zip(entries).enumerate() {
                    let selected = self.project_field(
                        Value::new(value.expr.clone(), value.ty.clone()),
                        index,
                        Some(field.name.clone()),
                        span,
                    )?;
                    let modeled = self.apply_model(entry, selected, span)?;
                    fields.push((field.name.clone(), modeled.expr));
                }
                Ok(Value::new(
                    Expr::Struct {
                        indices: Vec::new(),
                        id: info.id,
                        name: info.name.clone(),
                        fields,
                    },
                    Type::Struct(info.id),
                ))
            }
            Implementation::Definition(info) => {
                self.call_fn_with(info, &[Argument::Value(Box::new(value), span)], span)
            }
        }
    }
}
