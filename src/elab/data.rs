//! Building data and taking it apart: tuples, struct literals, enum
//! variants, and the fields of a struct or a tuple.

use crate::ast::{self, ExprKind, PatternKind};
use crate::kernel::{Term, Type, VarId, telescope_entry};
use crate::source::Span;
use crate::typed::{Binder, Expr};

use super::env::{Elab, EnumInfo, Env, Global};
use super::exprs::Value;

impl Env<'_> {
    pub(super) fn tuple(
        &mut self,
        items: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let telescope = match expected {
            Some(Type::Tuple(fields)) if fields.len() == items.len() => expected.cloned(),
            _ => None,
        };
        let mut fields = Vec::new();
        let mut terms = Vec::new();
        let mut tys = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let value = match &telescope {
                Some(telescope) => {
                    let ty = telescope_entry(telescope, index, &terms)
                        .expect("the index is within the telescope");
                    self.check(item, &ty)?
                }
                None => self.infer(item)?,
            };
            terms.push(self.term(&value, item.span)?);
            tys.push(value.ty);
            fields.push(value.expr);
        }
        let ty = telescope.unwrap_or(Type::Tuple(tys));
        let _ = span;
        Ok(Value::new(
            Expr::Tuple {
                ty: ty.clone(),
                fields,
            },
            ty,
        ))
    }

    pub(super) fn struct_literal(
        &mut self,
        name: &ast::Name,
        fields: &[ast::ValueField],
        span: Span,
    ) -> Elab<Value> {
        let Some(Global::Struct(info)) = self.types.get(&name.text).cloned() else {
            if self.failed.contains(&name.text) {
                return Err(());
            }
            return self.fail(
                "L0204",
                format!("unknown struct `{}`", name.text),
                name.span,
            );
        };
        let what = format!("`{}`", info.name);
        let values = self.values_by_name(&what, &info.fields, fields, span)?;
        let mut exprs = Vec::new();
        let mut tys: Vec<Type> = info.fields.iter().map(|field| field.ty.clone()).collect();
        for (index, (declared, value)) in info.fields.iter().zip(values).enumerate() {
            let value = self.check(value, &tys[index].clone())?;
            let term = self.term(&value, span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(declared.id, &term);
            }
            exprs.push((declared.name.clone(), value.expr));
        }
        Ok(Value::new(
            Expr::Struct {
                id: info.id,
                name: info.name.clone(),
                fields: exprs,
            },
            Type::Struct(info.id),
        ))
    }

    /// The values of `what { a: x, b }` by the declared fields' positions:
    /// each declared field once, in any order, `b` short for `b: b`.
    pub(super) fn values_by_name<'e>(
        &mut self,
        what: &str,
        declared: &[Binder],
        fields: &'e [ast::ValueField],
        span: Span,
    ) -> Elab<Vec<&'e ast::Expr>> {
        let mut given: Vec<(&str, &'e ast::Expr)> = Vec::new();
        for field in fields {
            let name = match (&field.name, &field.value.kind) {
                (Some(name), _) => name.text.as_str(),
                (None, ExprKind::Name(name)) => name.text.as_str(),
                (None, _) => {
                    return self.fail("L0224", "a field needs a name: `name: value`", field.span);
                }
            };
            self.declared_field(what, declared, name, &given, field.span)?;
            given.push((name, &field.value));
        }
        declared
            .iter()
            .map(
                |field| match given.iter().find(|(name, _)| *name == field.name) {
                    Some((_, value)) => Ok(*value),
                    None => {
                        let message = format!("missing field `{}` of {what}", field.name);
                        self.fail("L0224", message, span)
                    }
                },
            )
            .collect()
    }

    /// A field named where `what`'s fields are written or matched: one of
    /// the declared ones, not yet given.
    fn declared_field<T>(
        &mut self,
        what: &str,
        declared: &[Binder],
        name: &str,
        given: &[(&str, T)],
        span: Span,
    ) -> Elab<()> {
        if given.iter().any(|(earlier, _)| *earlier == name) {
            return self.fail("L0224", format!("field `{name}` is given twice"), span);
        }
        if !declared.iter().any(|field| field.name == name) {
            return self.fail("L0224", format!("{what} has no field `{name}`"), span);
        }
        Ok(())
    }

    /// A variant written the way it was declared: `V(..)` for a tuple
    /// variant and `V { .. }` for one with named fields.
    pub(super) fn variant_shape(
        &mut self,
        what: &str,
        declared: &[Binder],
        named: bool,
        braces: bool,
        span: Span,
    ) -> Elab<()> {
        if named == braces {
            return Ok(());
        }
        let fields: Vec<String> = declared
            .iter()
            .map(|field| format!("{}: ..", field.name))
            .collect();
        let message = if named {
            format!(
                "{what} has named fields and is written `{} {{ {} }}`",
                what.trim_matches('`'),
                fields.join(", ")
            )
        } else if declared.is_empty() {
            format!(
                "{what} has no fields and is written `{}`",
                what.trim_matches('`')
            )
        } else {
            format!(
                "{what} has no field names and is written `{}(..)`",
                what.trim_matches('`')
            )
        };
        self.fail("L0224", message, span)
    }

    /// The names an arm's pattern binds to a variant's fields, by position:
    /// `None` for `_`, and for a field a `..` leaves out. `V(a, _)` for a
    /// tuple variant, `V { a, b: x, .. }` for one with named fields, and `_`
    /// for either.
    pub(super) fn pattern_names<'p>(
        &mut self,
        pattern: &'p ast::Pattern,
        what: &str,
        declared: &[Binder],
        named: bool,
    ) -> Elab<Vec<Option<&'p ast::Name>>> {
        match &pattern.kind {
            PatternKind::Variant { arguments, path } => {
                self.variant_shape(what, declared, named, false, path.span)?;
                let given = arguments.as_deref().unwrap_or(&[]);
                if given.len() != declared.len() {
                    let message = format!(
                        "{what} carries {} value(s), and the pattern names {}",
                        declared.len(),
                        given.len()
                    );
                    return self.fail("L0208", message, path.span);
                }
                given
                    .iter()
                    .map(|pattern| self.pattern_name(pattern))
                    .collect()
            }
            PatternKind::Struct { path, fields, rest } => {
                self.variant_shape(what, declared, named, true, path.span)?;
                let mut given: Vec<(&str, Option<&'p ast::Name>)> = Vec::new();
                for field in fields {
                    let (name, bound) = match (&field.name, &field.pattern.kind) {
                        (Some(name), _) => (name.text.as_str(), self.pattern_name(&field.pattern)?),
                        (
                            None,
                            PatternKind::Name {
                                name,
                                mutable: false,
                            },
                        ) => (name.text.as_str(), Some(name)),
                        (None, _) => {
                            return self.fail(
                                "L0224",
                                "a field pattern is `name`, `name: other`, or `name: _`",
                                field.span,
                            );
                        }
                    };
                    self.declared_field(what, declared, name, &given, field.span)?;
                    given.push((name, bound));
                }
                declared
                    .iter()
                    .map(|field| match given.iter().find(|(name, _)| *name == field.name) {
                        Some((_, bound)) => Ok(*bound),
                        None if rest.is_some() => Ok(None),
                        None => {
                            let message = format!(
                                "the pattern leaves out field `{}` of {what}; `..` leaves out the rest",
                                field.name
                            );
                            self.fail("L0224", message, pattern.span)
                        }
                    })
                    .collect()
            }
            _ => Ok(vec![None; declared.len()]),
        }
    }

    /// A name or `_` inside a variant pattern.
    fn pattern_name<'p>(&mut self, pattern: &'p ast::Pattern) -> Elab<Option<&'p ast::Name>> {
        match &pattern.kind {
            PatternKind::Name {
                name,
                mutable: false,
            } => Ok(Some(name)),
            PatternKind::Wildcard => Ok(None),
            _ => self.fail(
                "L0290",
                "patterns inside a variant are names or `_` for now",
                pattern.span,
            ),
        }
    }

    pub(super) fn member(
        &mut self,
        expr: &ast::Expr,
        value: &ast::Expr,
        name: &ast::Name,
    ) -> Elab<Value> {
        let target = self.infer(value)?;
        let Type::Struct(id) = &target.ty else {
            let shown = self.show_type(&target.ty);
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error(
                    "L0210",
                    format!("`{shown}` has no field `{}`", name.text),
                    name.span,
                )
                .note(
                    "a tuple is projected by position, as in `pair.0`, or taken apart with `let`",
                ),
            );
            return Err(());
        };
        let info = self.struct_by_id(*id).expect("a struct type was declared");
        let Some(index) = info.fields.iter().position(|field| field.name == name.text) else {
            let message = format!("`{}` has no field `{}`", info.name, name.text);
            return self.fail("L0210", message, name.span);
        };
        self.field(target, index, Some(name.text.clone()), expr.span)
    }

    pub(super) fn index(
        &mut self,
        expr: &ast::Expr,
        value: &ast::Expr,
        index: &str,
        index_span: &Span,
    ) -> Elab<Value> {
        let target = self.infer(value)?;
        let arity = match &target.ty {
            Type::Tuple(fields) => fields.len(),
            _ => 0,
        };
        match index.parse::<usize>() {
            Ok(position) if position < arity => self.field(target, position, None, expr.span),
            _ => {
                let shown = self.show_type(&target.ty);
                self.fail(
                    "L0210",
                    format!("`{shown}` has no field `{index}`"),
                    *index_span,
                )
            }
        }
    }

    fn field(
        &mut self,
        target: Value,
        index: usize,
        name: Option<String>,
        span: Span,
    ) -> Elab<Value> {
        let term = self.term(&target, span)?;
        // The kernel knows what the field's type says about the other fields.
        let ty = self.type_of(&Term::proj(term, index), span)?;
        Ok(Value::new(
            Expr::Field {
                target: Box::new(target.expr),
                index,
                name,
                ty: ty.clone(),
            },
            ty,
        ))
    }

    /// The `Prefix::name` of a path. A path through modules, or one that
    /// begins with `crate`, `super`, or `self`, is not in Locus yet.
    pub(super) fn variant_path<'p>(
        &mut self,
        path: &'p ast::Path,
    ) -> Elab<(&'p ast::Name, &'p ast::Name)> {
        let root = &path.segments[0].text;
        if matches!(root.as_str(), "crate" | "super" | "self") {
            return self.fail(
                "L0290",
                format!("`{root}::` paths are not in Locus yet; modules are a later project"),
                path.span,
            );
        }
        match path.pair() {
            Some(pair) => Ok(pair),
            None => self.fail(
                "L0290",
                "paths through modules are not in Locus yet; modules are a later project",
                path.span,
            ),
        }
    }

    pub(super) fn enum_variant(
        &mut self,
        path: &ast::Path,
    ) -> Elab<Option<(std::rc::Rc<EnumInfo>, usize)>> {
        let (prefix, name) = self.variant_path(path)?;
        let global = self
            .types
            .get(&prefix.text)
            .or_else(|| self.values.get(&prefix.text))
            .cloned();
        match global {
            Some(Global::Enum(info)) => {
                match info
                    .variants
                    .iter()
                    .position(|variant| variant.name == name.text)
                {
                    Some(index) => Ok(Some((info, index))),
                    None => {
                        let message = format!("`{}` has no variant `{}`", info.name, name.text);
                        self.fail("L0212", message, name.span)
                    }
                }
            }
            Some(Global::Prop(_)) => Ok(None),
            Some(_) => self.fail(
                "L0212",
                format!("`{}` is not an enum or a proposition", prefix.text),
                prefix.span,
            ),
            None if self.failed.contains(&prefix.text) => Err(()),
            None => self.fail(
                "L0204",
                format!("unknown name `{}`", prefix.text),
                prefix.span,
            ),
        }
    }

    pub(super) fn variant(
        &mut self,
        path: &ast::Path,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let Some((info, index)) = self.enum_variant(path)? else {
            let Some(Global::Prop(info)) = self.types.get(&path.segments[0].text).cloned() else {
                unreachable!("`enum_variant` saw a proposition")
            };
            let arguments: Vec<&ast::Expr> = arguments.iter().collect();
            return self.construct(&info, path, &arguments, false, expected, span);
        };
        let variant = &info.variants[index];
        let what = format!("`{}::{}`", info.name, variant.name);
        self.variant_shape(&what, &variant.payload, variant.named, false, span)?;
        let arguments: Vec<&ast::Expr> = arguments.iter().collect();
        self.variant_value(&info, index, &arguments, span)
    }

    /// `E::V { a: x, b }`, a variant with named fields, or a proposition's.
    pub(super) fn variant_literal(
        &mut self,
        path: &ast::Path,
        fields: &[ast::ValueField],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let Some((info, index)) = self.enum_variant(path)? else {
            let Some(Global::Prop(info)) = self.types.get(&path.segments[0].text).cloned() else {
                unreachable!("`enum_variant` saw a proposition")
            };
            let variant = &info.variants[self.prop_variant(&info, path)?];
            let what = format!("`{}::{}`", info.name, variant.name);
            self.variant_shape(&what, &variant.payload, variant.named, true, span)?;
            let values = self.values_by_name(&what, &variant.payload, fields, span)?;
            return self.construct(&info, path, &values, true, expected, span);
        };
        let variant = &info.variants[index];
        let what = format!("`{}::{}`", info.name, variant.name);
        self.variant_shape(&what, &variant.payload, variant.named, true, span)?;
        let values = self.values_by_name(&what, &variant.payload, fields, span)?;
        self.variant_value(&info, index, &values, span)
    }

    fn variant_value(
        &mut self,
        info: &EnumInfo,
        index: usize,
        arguments: &[&ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        let variant = &info.variants[index];
        let ids: Vec<VarId> = variant.payload.iter().map(|binder| binder.id).collect();
        let mut tys: Vec<Type> = variant
            .payload
            .iter()
            .map(|binder| binder.ty.clone())
            .collect();
        let what = format!("`{}::{}`", info.name, variant.name);
        let payload = self.arguments_by_ref(arguments, &ids, &mut tys, &what, span)?;
        Ok(Value::new(
            Expr::Variant {
                id: info.id,
                enum_name: info.name.clone(),
                index,
                variant_name: variant.name.clone(),
                payload,
            },
            Type::Enum(info.id),
        ))
    }
}
