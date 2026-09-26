//! Structural traversal for checked spec lowering. No expression is evaluated here.
use crate::ast::*;
use crate::source::Span;

pub trait Walk {
    fn span(&mut self, _: &mut Span) {}
    fn name(&mut self, n: &mut Name) {
        self.span(&mut n.span);
    }
    fn path(&mut self, p: &mut Path) {
        self.span(&mut p.span);
        for n in &mut p.segments {
            self.name(n);
        }
    }
    fn ty(&mut self, t: &mut Type) {
        ty(self, t);
    }
    fn expr(&mut self, e: &mut Expr) {
        expr(self, e);
    }
    fn block(&mut self, b: &mut Block) {
        block(self, b);
    }
    fn pattern(&mut self, p: &mut Pattern) {
        pattern(self, p);
    }
}
pub fn parameter<W: Walk + ?Sized>(w: &mut W, p: &mut Parameter) {
    w.ty(&mut p.ty);
    w.name(&mut p.name);
    w.span(&mut p.span);
}
pub fn fields<W: Walk + ?Sized>(w: &mut W, fs: &mut [TypeField]) {
    for f in fs {
        w.ty(&mut f.ty);
        if let Some(n) = &mut f.name {
            w.name(n);
        }
        w.span(&mut f.span);
    }
}
pub fn ty<W: Walk + ?Sized>(w: &mut W, t: &mut Type) {
    w.span(&mut t.span);
    match &mut t.kind {
        TypeKind::Dyn(bound) => {
            w.path(&mut bound.path);
            for (_, ty) in &mut bound.associated {
                w.ty(ty);
            }
        }
        TypeKind::Scoped { name, claims } => {
            w.name(name);
            for claim in claims {
                w.expr(claim);
            }
        }
        TypeKind::Named(n) | TypeKind::Lifetime(n) => w.name(n),
        TypeKind::Path { path, arguments } => {
            w.path(path);
            for a in arguments {
                w.ty(a);
            }
        }
        TypeKind::Group(t) | TypeKind::Slice(t) => w.ty(t),
        TypeKind::Ref {
            lifetime, inner, ..
        } => {
            if let Some(n) = lifetime {
                w.name(n);
            }
            w.ty(inner);
        }
        TypeKind::Array { element, length } => {
            w.ty(element);
            w.expr(length);
        }
        TypeKind::Tuple(fs) => fields(w, fs),
        TypeKind::Proof(e) => w.expr(e),
        TypeKind::Function { parameters, result }
        | TypeKind::LogicalFunction { parameters, result } => {
            fields(w, parameters);
            w.ty(result);
        }
        TypeKind::Unit | TypeKind::Never => {}
    }
}
pub fn block<W: Walk + ?Sized>(w: &mut W, b: &mut Block) {
    w.span(&mut b.span);
    for s in &mut b.statements {
        w.span(&mut s.span);
        match &mut s.kind {
            StatementKind::Let {
                pattern,
                annotation,
                value,
                ..
            } => {
                w.expr(value);
                if let Some(t) = annotation {
                    w.ty(t);
                }
                w.pattern(pattern);
            }
            StatementKind::Assign { place, value } => {
                w.expr(value);
                w.expr(place);
            }
            StatementKind::Expression(e) => w.expr(e),
            StatementKind::Error => {}
        }
    }
    if let Some(e) = &mut b.tail {
        w.expr(e);
    }
}
pub fn pattern<W: Walk + ?Sized>(w: &mut W, p: &mut Pattern) {
    w.span(&mut p.span);
    match &mut p.kind {
        PatternKind::Name { name, .. } => w.name(name),
        PatternKind::Binding {
            name,
            pattern,
            at_span,
            ..
        } => {
            w.name(name);
            w.pattern(pattern);
            w.span(at_span);
        }
        PatternKind::Evidence {
            constructor,
            evidence,
            at_span,
        } => {
            w.pattern(constructor);
            w.pattern(evidence);
            w.span(at_span);
        }
        PatternKind::Group(p) => w.pattern(p),
        PatternKind::Tuple(ps) => {
            for p in ps {
                w.pattern(p);
            }
        }
        PatternKind::Struct { path, fields, rest } => {
            w.path(path);
            for f in fields {
                if let Some(n) = &mut f.name {
                    w.name(n);
                }
                w.pattern(&mut f.pattern);
                w.span(&mut f.span);
            }
            if let Some(s) = rest {
                w.span(s);
            }
        }
        PatternKind::Variant { path, arguments } => {
            w.path(path);
            if let Some(ps) = arguments {
                for p in ps {
                    w.pattern(p);
                }
            }
        }
        _ => {}
    }
}
pub fn expr<W: Walk + ?Sized>(w: &mut W, e: &mut Expr) {
    w.span(&mut e.span);
    match &mut e.kind {
        ExprKind::Scoped { value, ty, .. } => {
            w.expr(value);
            w.ty(ty);
        }
        ExprKind::Name(n) => w.name(n),
        ExprKind::Path(p) => w.path(p),
        ExprKind::Group(e) | ExprKind::Not(e) => w.expr(e),
        ExprKind::Tuple(es) | ExprKind::Array(es) => {
            for e in es {
                w.expr(e);
            }
        }
        ExprKind::Subscript { value, index } => {
            w.expr(value);
            w.expr(index);
        }
        ExprKind::Form {
            name_span,
            arguments,
            source_hint,
            ..
        } => {
            w.span(name_span);
            for e in arguments {
                w.expr(e);
            }
            if let Some(t) = source_hint {
                w.ty(t);
            }
        }
        ExprKind::Struct { path, fields } => {
            w.path(path);
            for f in fields {
                if let Some(n) = &mut f.name {
                    w.name(n);
                }
                w.expr(&mut f.value);
                w.span(&mut f.span);
            }
        }
        ExprKind::Block(b) | ExprKind::Logic(b) | ExprKind::Loop { body: b } => w.block(b),
        ExprKind::Evidence {
            constructor,
            evidence,
            at_span,
        } => {
            w.expr(constructor);
            w.expr(evidence);
            w.span(at_span);
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            w.expr(condition);
            w.block(then_branch);
            w.expr(else_branch);
        }
        ExprKind::Match { scrutinee, arms } => {
            w.expr(scrutinee);
            for a in arms {
                w.pattern(&mut a.pattern);
                w.expr(&mut a.body);
                w.span(&mut a.span);
            }
        }
        ExprKind::While {
            pattern,
            condition,
            body,
        } => {
            w.expr(condition);
            if let Some(p) = pattern {
                w.pattern(p);
            }
            w.block(body);
        }
        ExprKind::For {
            pattern,
            iterable,
            body,
        } => {
            w.expr(iterable);
            w.pattern(pattern);
            w.block(body);
        }
        ExprKind::Range { lower, upper, .. } => {
            w.expr(lower);
            w.expr(upper);
        }
        ExprKind::Break(e) | ExprKind::Return(e) => {
            if let Some(e) = e {
                w.expr(e);
            }
        }
        ExprKind::Ref { expr, .. } => w.expr(expr),
        ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
            for p in parameters {
                parameter(w, p);
            }
            w.block(body);
        }
        ExprKind::Closure { parameters, body } => {
            for p in parameters {
                parameter(w, p);
            }
            w.expr(body);
        }
        ExprKind::Unary {
            operator_span,
            expr,
            ..
        } => {
            w.span(operator_span);
            w.expr(expr);
        }
        ExprKind::Binary {
            operator_span,
            left,
            right,
            ..
        } => {
            w.span(operator_span);
            w.expr(left);
            w.expr(right);
        }
        ExprKind::Cast {
            expr,
            source_hint,
            as_span,
            ty,
        } => {
            w.expr(expr);
            if let Some(t) = source_hint {
                w.ty(t);
            }
            w.span(as_span);
            w.ty(ty);
        }
        ExprKind::Call { callee, arguments } => {
            w.expr(callee);
            for e in arguments {
                w.expr(e);
            }
        }
        ExprKind::GenericApply { callee, arguments } => {
            w.expr(callee);
            for t in arguments {
                w.ty(t);
            }
        }
        ExprKind::Member { value, name } => {
            w.expr(value);
            w.name(name);
        }
        ExprKind::Index {
            value, index_span, ..
        } => {
            w.expr(value);
            w.span(index_span);
        }
        _ => {}
    }
}
pub fn member<W: Walk + ?Sized>(w: &mut W, d: &mut Declaration) {
    w.span(&mut d.span);
    for predicate in &mut d.constraints {
        w.ty(&mut predicate.subject);
        w.span(&mut predicate.span);
        for bound in &mut predicate.bounds {
            w.path(&mut bound.path);
            for (name, ty) in &mut bound.associated {
                w.name(name);
                w.ty(ty);
            }
        }
    }
    for a in &mut d.attributes {
        w.span(&mut a.span);
        if let AttributeKind::Terminates { decreases: Some(e) } = &mut a.kind {
            w.expr(e);
        }
    }
    match &mut d.kind {
        DeclarationKind::Function {
            generics,
            name,
            self_param,
            parameters,
            result,
            body,
            ..
        } => {
            for g in generics {
                w.name(&mut g.name);
                w.span(&mut g.span);
                for p in &mut g.bounds {
                    w.path(p);
                    for (name, ty) in &mut p.associated {
                        w.name(name);
                        w.ty(ty);
                    }
                }
            }
            w.name(name);
            if let Some(p) = self_param {
                w.span(&mut p.span);
            }
            for p in parameters {
                parameter(w, p);
            }
            w.ty(result);
            w.block(body);
        }
        DeclarationKind::Constant { name, ty, value } => {
            w.name(name);
            w.ty(ty);
            w.expr(value);
        }
        DeclarationKind::Struct { fields, .. } => {
            for f in fields {
                w.ty(&mut f.ty);
            }
        }
        DeclarationKind::Enum { variants, .. } => {
            for v in variants {
                fields(w, &mut v.fields);
            }
        }
        DeclarationKind::Prop {
            parameters,
            variants,
            ..
        } => {
            for p in parameters {
                parameter(w, p);
            }
            for v in variants {
                fields(w, &mut v.fields);
                if let Some(e) = &mut v.target {
                    w.expr(e);
                }
                if let Some(b) = &mut v.body {
                    w.block(b);
                }
            }
        }
        DeclarationKind::Spec { members, .. }
        | DeclarationKind::Trait { members, .. }
        | DeclarationKind::SpecImpl { members, .. } => {
            for m in members {
                member(w, m);
            }
        }
        DeclarationKind::AssociatedType { name, value, .. } => {
            w.name(name);
            if let Some(t) = value {
                w.ty(t);
            }
        }
        _ => {}
    }
}
