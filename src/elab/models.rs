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
    BooleanView,
    Definition(Rc<FnInfo>),
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

pub(super) fn primitive_models() -> Vec<ModelEntry> {
    let mut models: Vec<_> = MachineInt::ALL
        .into_iter()
        .map(|machine| ModelEntry {
            source: Type::machine(machine),
            target: Type::Int,
            source_layout: ErasureLayout::Default,
            implementation: Implementation::IntegerView,
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
                "a Model implementation contains exactly one `logic fn model(source: &T) -> Self`",
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
                "a Model implementation must define `logic fn model(source: &T) -> Self`",
                method.span,
            );
        };
        if self.text(name.span) != "model"
            || !generics.is_empty()
            || parameters.len() != 1
            || !matches!(
                parameters[0].ty.kind,
                ast::TypeKind::Ref { mutable: false, .. }
            )
        {
            return self.fail("L0282", "the Model method takes one shared source reference: `logic fn model(source: &T) -> Self`", method.span);
        }
        Ok(())
    }

    pub(super) fn register_model(&mut self, model: &ast::ModelImpl, info: Rc<FnInfo>) -> Elab<()> {
        let source = match &model.source.kind {
            ast::TypeKind::Slice(element) => self.collection_type(element, model.source.span)?,
            _ => self.ty(&model.source)?,
        };
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
            || info.passing != [Passing::Ref]
            || info.params[0].ty != source
            || info.result != target
        {
            return self.fail("L0282", "the Model method must take the declared source by shared reference and return the declared logical destination", model.span);
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
                && entry.target == target
                && (super::layout::borrow_compatible(&source, &source_layout, &entry.source_layout)
                    || super::layout::borrow_compatible(
                        &source,
                        &entry.source_layout,
                        &source_layout,
                    ))
        }) {
            let mut diagnostic = Diagnostic::error(
                "L0282",
                "this source/destination pair already has an overlapping Model implementation",
                model.span,
            );
            if let Some(span) = earlier.span {
                diagnostic = diagnostic.label(span, "first implementation");
            } else {
                diagnostic =
                    diagnostic.note("the pair has a primitive model supplied by the compiler");
            }
            self.diagnostics.push(diagnostic);
            return Err(());
        }
        self.models.push(ModelEntry {
            source,
            target,
            source_layout,
            implementation: Implementation::Definition(info),
            span: Some(model.span),
            default: false,
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
        // Identity on an already logical value is handled by ordinary casts;
        // crossing from physical data always selects a registered model.
        let (entry, value) = self.model_source(inner, target, span)?;
        self.apply_model(&entry, value, inner.span)
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
            .find(|entry| entry.default && entry.source == value.ty)
            .cloned();
        match entry {
            Some(entry) => self.apply_model(&entry, value, span),
            None => Ok(value),
        }
    }

    fn apply_model(&mut self, entry: &ModelEntry, value: Value, span: Span) -> Elab<Value> {
        match &entry.implementation {
            Implementation::Identity => Ok(value),
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
            Implementation::Definition(info) => {
                self.call_fn_with(info, &[Argument::Value(Box::new(value), span)], span)
            }
        }
    }
}
