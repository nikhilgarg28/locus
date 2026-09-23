//! Surface erasure shapes retain Bool/bool distinctions inside products.
//! They accompany kernel types; they never alter a proposition or proof.
use super::{
    env::{Elab, Env},
    exprs::Value,
};
use crate::{
    ast,
    kernel::Type,
    source::Span,
    typed::{ErasureLayout, Pattern},
};

impl Env<'_> {
    pub(super) fn written_layout(&self, ty: &ast::Type) -> ErasureLayout {
        if self.logical_spelling(ty) {
            return ErasureLayout::Logical;
        }
        match &ty.kind {
            ast::TypeKind::Path { path, arguments }
                if path.single().is_some_and(|n| n.text == "Box") && arguments.len() == 1 =>
            {
                ErasureLayout::Boxed(Box::new(self.written_layout(&arguments[0])))
            }
            ast::TypeKind::Group(inner) => self.written_layout(inner),
            ast::TypeKind::Ref {
                lifetime, inner, ..
            } => ErasureLayout::Shared {
                lifetime: lifetime.as_ref().map(|l| l.text.clone()),
                inner: Box::new(self.written_layout(inner)),
            },
            ast::TypeKind::Path { path, arguments }
                if path.single().is_some_and(|name| name.text == "Vec") && arguments.len() == 1 =>
            {
                ErasureLayout::Buffer {
                    storage: crate::exec::BufferStorage::Vector,
                    element: Box::new(self.written_layout(&arguments[0])),
                }
            }
            ast::TypeKind::Path { path, arguments }
                if !arguments.is_empty()
                    && arguments
                        .iter()
                        .all(|a| matches!(a.kind, ast::TypeKind::Lifetime(_))) =>
            {
                let args = arguments
                    .iter()
                    .filter_map(|a| match &a.kind {
                        ast::TypeKind::Lifetime(n) => Some(n.text.clone()),
                        _ => None,
                    })
                    .collect();
                let nominal = path
                    .single()
                    .and_then(|name| self.types.get(&self.type_text(name)))
                    .and_then(|item| match item {
                        super::env::Global::Struct(s) => Some(Type::Struct(s.id)),
                        super::env::Global::Enum(e) => Some(Type::Enum(e.id)),
                        _ => None,
                    });
                ErasureLayout::NominalLifetimes(match nominal {
                    Some(ty) => self.session.canonical_lifetime_arguments(&ty, args),
                    None => args,
                })
            }
            ast::TypeKind::Array { element, length } => {
                let n = match &length.kind {
                    ast::ExprKind::Integer(n) => n
                        .value
                        .to_u128()
                        .and_then(|n| usize::try_from(n).ok())
                        .unwrap_or(0),
                    _ => 0,
                };
                ErasureLayout::Buffer {
                    storage: crate::exec::BufferStorage::Array(n),
                    element: Box::new(self.written_layout(element)),
                }
            }
            ast::TypeKind::Slice(element) => ErasureLayout::Buffer {
                storage: crate::exec::BufferStorage::Slice,
                element: Box::new(self.written_layout(element)),
            },
            ast::TypeKind::Tuple(fields) => ErasureLayout::Tuple(
                fields
                    .iter()
                    .map(|field| self.written_layout(&field.ty))
                    .collect(),
            ),
            _ => ErasureLayout::Default,
        }
    }

    /// Put contextual shapes on expressions which provide the value of an
    /// expression. Arguments and conditions have their own expectations.
    pub(super) fn expect_layout(&mut self, expr: &ast::Expr, layout: &ErasureLayout) {
        self.layout_hints.insert(expr.span, layout.clone());
        match &expr.kind {
            ast::ExprKind::Group(inner) => self.expect_layout(inner, layout),
            ast::ExprKind::Tuple(fields) => {
                for (index, field) in fields.iter().enumerate() {
                    self.expect_layout(field, &layout.field(index));
                }
            }
            ast::ExprKind::Block(block) | ast::ExprKind::Logic(block) => {
                self.expect_block_layout(block, layout)
            }
            ast::ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.expect_block_layout(then_branch, layout);
                self.expect_layout(else_branch, layout);
            }
            ast::ExprKind::Match { arms, .. } => {
                for arm in arms {
                    self.expect_layout(&arm.body, layout);
                }
            }
            _ => {}
        }
    }

    pub(super) fn expect_block_layout(&mut self, block: &ast::Block, layout: &ErasureLayout) {
        if let Some(tail) = block.tail.as_deref() {
            self.expect_layout(tail, layout);
        }
    }

    pub(super) fn check_value_layout(&mut self, mut value: Value, span: Span) -> Elab<Value> {
        if value.never {
            return Ok(value);
        }
        let branch_layouts: Vec<_> = match &value.expr {
            crate::typed::Expr::If {
                then_block,
                else_block,
                ..
            } => [then_block, else_block]
                .iter()
                .filter_map(|block| block.tail.as_deref())
                .map(|tail| self.session.expression_layout(tail))
                .collect(),
            crate::typed::Expr::Match { arms, .. } => arms
                .iter()
                .filter_map(|arm| arm.body.tail.as_deref())
                .map(|tail| self.session.expression_layout(tail))
                .collect(),
            _ => Vec::new(),
        };
        if let Some(first) = branch_layouts.first()
            && branch_layouts
                .iter()
                .skip(1)
                .any(|other| !compatible(&value.ty, other, first))
        {
            return self.fail(
                "L0272",
                "branches must agree on logical and runtime positions in their result",
                span,
            );
        }
        let actual = self.session.expression_layout(&value.expr);
        if let Some(expected) = self.layout_hints.get(&span).cloned() {
            if expected.is_logical() && matches!(value.ty, Type::Bool) {
                value.expr = super::calls::ghost_value(value.expr);
            } else if !compatible(&value.ty, &actual, &expected) {
                return self.fail(
                    "L0272",
                    "logical and runtime positions in this value do not match the declared type",
                    span,
                );
            }
        } else if actual.is_logical() && matches!(value.ty, Type::Bool) {
            value.expr = super::calls::ghost_value(value.expr);
        }
        Ok(value)
    }

    pub(super) fn written_parameter_layout(&self, ty: &ast::Type) -> ErasureLayout {
        match &ty.kind {
            ast::TypeKind::Ref {
                lifetime: Some(lifetime),
                inner,
                ..
            } => ErasureLayout::Borrowed {
                lifetime: Some(lifetime.text.clone()),
                inner: Box::new(self.written_layout(inner)),
            },
            ast::TypeKind::Ref { inner, .. } => self.written_layout(inner),
            _ => self.written_layout(ty),
        }
    }

    pub(super) fn register_pattern_layout(&mut self, pattern: &Pattern, layout: &ErasureLayout) {
        match pattern {
            Pattern::Bind { binder, .. } => self
                .session
                .register_binding_layout(binder.id, layout.clone()),
            Pattern::Tuple(fields) => {
                for (index, field) in fields.iter().enumerate() {
                    self.register_pattern_layout(field, &layout.field(index));
                }
            }
            Pattern::Wildcard => {}
        }
    }
}

pub(super) fn compatible(ty: &Type, actual: &ErasureLayout, expected: &ErasureLayout) -> bool {
    match (actual, expected) {
        (ErasureLayout::Borrowed { inner, .. }, _) => return compatible(ty, inner, expected),
        (_, ErasureLayout::Borrowed { inner, .. }) => return compatible(ty, actual, inner),
        (ErasureLayout::Shared { inner: a, .. }, ErasureLayout::Shared { inner: b, .. }) => {
            return compatible(ty, a, b);
        }
        (ErasureLayout::Shared { .. }, _) | (_, ErasureLayout::Shared { .. }) => return false,
        _ => {}
    }
    match ty {
        Type::Buffer(element) => match (actual, expected) {
            (
                ErasureLayout::Buffer {
                    storage: a,
                    element: al,
                },
                ErasureLayout::Buffer {
                    storage: b,
                    element: bl,
                },
            ) => a == b && compatible(element, al, bl),
            _ => actual == expected,
        },
        Type::Bool => actual.is_logical() == expected.is_logical(),
        Type::Tuple(fields) => fields
            .iter()
            .enumerate()
            .all(|(index, field)| compatible(field, &actual.field(index), &expected.field(index))),
        _ => true,
    }
}

/// Rust's ordinary array/vector-to-slice coercion is available only while
/// lending storage. It never changes the layout of an owned value.
pub(super) fn borrow_compatible(
    ty: &Type,
    actual: &ErasureLayout,
    expected: &ErasureLayout,
) -> bool {
    if let ErasureLayout::Borrowed { inner, .. } = expected {
        return borrow_compatible(ty, actual, inner);
    }
    if let ErasureLayout::Shared { inner, .. } = actual
        && !matches!(expected, ErasureLayout::Shared { .. })
    {
        return borrow_compatible(ty, inner, expected);
    }
    if let (
        Type::Buffer(element),
        ErasureLayout::Buffer {
            storage: _,
            element: a,
        },
        ErasureLayout::Buffer {
            storage: crate::exec::BufferStorage::Slice,
            element: b,
        },
    ) = (ty, actual, expected)
    {
        return compatible(element, a, b);
    }
    compatible(ty, actual, expected)
}
