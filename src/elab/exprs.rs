//! Expressions and blocks to typed trees.
//!
//! Elaboration is bidirectional: an expression is elaborated against the
//! type expected of it when there is one, which is how a `_` learns what to
//! prove and how a tuple learns that its second field speaks of its first.

use crate::ast::{self, BinaryOp, ExprKind, PatternKind, StatementKind};
use crate::kernel::{
    HypId, Prim, Proof, Term, Type, VarId, case_variants, same_type, telescope_entry, variant_term,
};
use crate::source::Span;
use crate::typed::{
    self, Binder, CompareOp, Expr, FnRef, MatchArm, Pattern, Stmt, is_pure, value_term,
};

use super::env::{Elab, EnumInfo, Env, FnInfo, Global, LoopTarget, PropInfo};
use super::types::tuple_over;

pub(super) struct Value {
    pub expr: Expr,
    pub ty: Type,
    /// The expression transfers control and produces no value.
    pub never: bool,
}

impl Value {
    pub fn new(expr: Expr, ty: Type) -> Self {
        Self {
            expr,
            ty,
            never: false,
        }
    }
}

fn unit_type() -> Type {
    Type::Tuple(Vec::new())
}

impl Env<'_> {
    /// The kernel term that stands for the value, as lowering will state it.
    pub fn term(&mut self, value: &Value, span: Span) -> Elab<Term> {
        match value_term(&value.expr) {
            Ok(term) => Ok(term),
            Err(error) => self.internal(error, span),
        }
    }

    pub fn infer(&mut self, expr: &ast::Expr) -> Elab<Value> {
        self.expr(expr, None)
    }

    pub fn check(&mut self, expr: &ast::Expr, expected: &Type) -> Elab<Value> {
        let value = self.expr(expr, Some(expected))?;
        self.coerce(value, expected, expr.span)
    }

    /// Accepts a value where `expected` is wanted. Evidence of one claim is
    /// accepted for another when the solver can bridge them, which is what
    /// lets a fact about `result.0` serve as a fact about the `next` it was
    /// bound to.
    pub fn coerce(&mut self, value: Value, expected: &Type, span: Span) -> Elab<Value> {
        if value.never || same_type(&value.ty, expected) {
            return Ok(value);
        }
        if let (Type::Proof(found), Type::Proof(wanted)) = (&value.ty, expected) {
            let mark = self.mark();
            let term = self.term(&value, span)?;
            self.facts.push(super::env::Fact {
                proof: Proof::OfTerm(term),
                claim: (**found).clone(),
            });
            let solved = self.solve(wanted, span, Some(found));
            self.close_names(mark);
            return Ok(Value::new(Expr::Proof(solved?), expected.clone()));
        }
        let (wanted, found) = (self.show_type(expected), self.show_type(&value.ty));
        self.fail(
            "L0220",
            format!("expected `{wanted}`, found `{found}`"),
            span,
        )
    }

    fn expr(&mut self, expr: &ast::Expr, expected: Option<&Type>) -> Elab<Value> {
        match &expr.kind {
            ExprKind::Error => Err(()),
            ExprKind::Group(inner) => self.expr(inner, expected),
            ExprKind::Unit => Ok(Value::new(Expr::unit(), unit_type())),
            ExprKind::Bool(value) => Ok(Value::new(Expr::Bool(*value), Type::Bool)),
            ExprKind::Integer(text) => match text.replace('_', "").parse::<u8>() {
                Ok(value) => Ok(Value::new(Expr::U8(value), Type::U8)),
                Err(_) => self.fail(
                    "L0205",
                    format!("`{text}` does not fit in `u8`, whose largest value is 255"),
                    expr.span,
                ),
            },
            ExprKind::Name(name) => self.name(name, expected),
            ExprKind::Hole => match expected {
                Some(Type::Proof(claim)) => {
                    let proof = self.solve(claim, expr.span, None)?;
                    Ok(Value::new(Expr::Proof(proof), Type::Proof(claim.clone())))
                }
                Some(other) => {
                    let shown = self.show_type(other);
                    self.fail(
                        "L0206",
                        format!("`_` asks for evidence, and a `{shown}` is needed here"),
                        expr.span,
                    )
                }
                None => {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0206",
                            "`_` asks for evidence, and nothing here says of what",
                            expr.span,
                        )
                        .note("state the claim with an annotation: `let evidence: @[claim] = _;`"),
                    );
                    Err(())
                }
            },
            ExprKind::Tuple(items) => self.tuple(items, expected, expr.span),
            ExprKind::Proposition(_)
            | ExprKind::Forall { .. }
            | ExprKind::Exists { .. }
            | ExprKind::Binary {
                operator: BinaryOp::Implies,
                ..
            } => {
                let term = self.formula(expr)?;
                Ok(Value::new(Expr::Prop(term), Type::Prop))
            }
            ExprKind::Not(_)
            | ExprKind::Binary {
                operator: BinaryOp::And | BinaryOp::Or,
                ..
            } if expected.is_some_and(|ty| same_type(ty, &Type::Prop))
                || self.reads_as_prop(expr) =>
            {
                let term = self.formula(expr)?;
                Ok(Value::new(Expr::Prop(term), Type::Prop))
            }
            ExprKind::Not(inner) => {
                let otherwise = ast::Expr {
                    kind: ExprKind::Bool(true),
                    span: expr.span,
                };
                let then = ast::Expr {
                    kind: ExprKind::Bool(false),
                    span: expr.span,
                };
                self.conditional(
                    inner,
                    Branch::Expr(&then),
                    Branch::Expr(&otherwise),
                    expected,
                    expr.span,
                )
            }
            ExprKind::Binary {
                operator: operator @ (BinaryOp::And | BinaryOp::Or),
                left,
                right,
                ..
            } => {
                // Short-circuit evaluation is an `if`.
                let constant = ast::Expr {
                    kind: ExprKind::Bool(*operator == BinaryOp::Or),
                    span: expr.span,
                };
                let (then, otherwise) = if *operator == BinaryOp::And {
                    (Branch::Expr(right), Branch::Expr(&constant))
                } else {
                    (Branch::Expr(&constant), Branch::Expr(right))
                };
                self.conditional(left, then, otherwise, Some(&Type::Bool), expr.span)
            }
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => {
                if expected.is_some_and(|ty| same_type(ty, &Type::Prop)) {
                    let term = self.formula(expr)?;
                    return Ok(Value::new(Expr::Prop(term), Type::Prop));
                }
                let (left_value, right_value) = self.operands(left, right)?;
                if !same_type(&left_value.ty, &Type::U8) {
                    let shown = self.show_type(&left_value.ty);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0211",
                            format!("values of type `{shown}` cannot be compared at runtime in the core"),
                            expr.span,
                        )
                        .note("runtime comparison is defined on `u8`; inside `[ ... ]`, `==` states equality at any type"),
                    );
                    return Err(());
                }
                let op = match operator {
                    BinaryOp::Equal => CompareOp::Eq,
                    BinaryOp::NotEqual => CompareOp::Ne,
                    BinaryOp::Less => CompareOp::Lt,
                    BinaryOp::LessEqual => CompareOp::Le,
                    BinaryOp::Greater => CompareOp::Gt,
                    _ => CompareOp::Ge,
                };
                Ok(Value::new(
                    Expr::Compare {
                        op,
                        left: Box::new(left_value.expr),
                        right: Box::new(right_value.expr),
                    },
                    Type::Bool,
                ))
            }
            ExprKind::Struct { name, fields } => self.struct_literal(name, fields, expr.span),
            ExprKind::Path(path) => self.variant(path, &[], expected, expr.span),
            ExprKind::Call { callee, arguments } => {
                self.call(callee, arguments, expected, expr.span)
            }
            ExprKind::Member { value, name } => {
                let target = self.infer(value)?;
                let Type::Struct(id) = &target.ty else {
                    let shown = self.show_type(&target.ty);
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0210",
                            format!("`{shown}` has no field `{}`", name.text),
                            name.span,
                        )
                        .note("a tuple is projected by position, as in `pair.0`, or taken apart with `let`"),
                    );
                    return Err(());
                };
                let info = self.struct_by_id(*id).expect("a struct type was declared");
                let Some(index) = info.fields.iter().position(|field| field.name == name.text)
                else {
                    let message = format!("`{}` has no field `{}`", info.name, name.text);
                    return self.fail("L0210", message, name.span);
                };
                self.field(target, index, Some(name.text.clone()), expr.span)
            }
            ExprKind::Index {
                value,
                index,
                index_span,
            } => {
                let target = self.infer(value)?;
                let arity = match &target.ty {
                    Type::Tuple(fields) => fields.len(),
                    _ => 0,
                };
                match index.parse::<usize>() {
                    Ok(position) if position < arity => {
                        self.field(target, position, None, expr.span)
                    }
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
            ExprKind::Block(block) => {
                let mark = self.mark();
                let result = self.block(block, expected);
                self.close_names(mark);
                let (block, ty, never) = result?;
                Ok(Value {
                    expr: Expr::Block(block),
                    ty,
                    never,
                })
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => self.conditional(
                condition,
                Branch::Block(then_branch),
                Branch::Expr(else_branch),
                expected,
                expr.span,
            ),
            ExprKind::Match { scrutinee, arms } => {
                self.match_(scrutinee, arms, expected, expr.span)
            }
            ExprKind::Loop {
                state,
                result,
                body,
            } => self.loop_(state, result, body, expr.span),
            ExprKind::For {
                index,
                lower,
                upper,
                state,
                body,
            } => self.for_(index, lower, upper, state, body, expr.span),
            ExprKind::Break(value) => {
                let Some(target) = self.loops.last().cloned() else {
                    return self.fail("L0217", "`break` outside a loop", expr.span);
                };
                let Some(result) = target.result else {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0218",
                            "a bounded `for` runs to the end of its range and has no `break`",
                            expr.span,
                        )
                        .note("carry a `bool` in the state to stop doing work early"),
                    );
                    return Err(());
                };
                let value = self.check(value, &result)?;
                Ok(Value {
                    expr: Expr::Break(Box::new(value.expr)),
                    ty: unit_type(),
                    never: true,
                })
            }
            ExprKind::Continue(arguments) => {
                let Some(target) = self.loops.last().cloned() else {
                    return self.fail("L0217", "`continue` outside a loop", expr.span);
                };
                let mut tys: Vec<Type> = target
                    .state
                    .iter()
                    .map(|binder| match &target.advance {
                        Some((index, next)) => binder.ty.replace_var(*index, next),
                        None => binder.ty.clone(),
                    })
                    .collect();
                let ids: Vec<VarId> = target.state.iter().map(|binder| binder.id).collect();
                let next =
                    self.arguments(arguments, &ids, &mut tys, "the loop's state", expr.span)?;
                Ok(Value {
                    expr: Expr::Continue(next),
                    ty: unit_type(),
                    never: true,
                })
            }
        }
    }

    fn name(&mut self, name: &ast::Name, expected: Option<&Type>) -> Elab<Value> {
        if let Some(local) = self.lookup(&name.text) {
            if local.poisoned {
                return Err(());
            }
            let (id, ty) = (local.id, local.ty.clone());
            return Ok(Value::new(
                Expr::Var {
                    id,
                    name: name.text.clone(),
                    ty: ty.clone(),
                },
                ty,
            ));
        }
        match self.globals.get(&name.text).cloned() {
            Some(Global::Fn(info)) if info.constant => self.call_fn(&info, &[], name.span),
            Some(Global::Fn(info)) if matches!(expected, Some(Type::Proof(_))) => {
                self.function_as_evidence(&info, name.span)
            }
            Some(Global::Fn(_)) => self.fail(
                "L0290",
                "a function used as a value is not supported yet; call it",
                name.span,
            ),
            Some(_) => self.fail(
                "L0204",
                format!("`{}` is not a value", name.text),
                name.span,
            ),
            None if self.failed.contains(&name.text) => Err(()),
            None => self.fail("L0204", format!("unknown name `{}`", name.text), name.span),
        }
    }

    /// Whether an expression written outside brackets is a proposition, as
    /// `p || q` is when `p` is a `Prop`.
    fn reads_as_prop(&self, expr: &ast::Expr) -> bool {
        match &expr.kind {
            ExprKind::Group(inner) | ExprKind::Not(inner) => self.reads_as_prop(inner),
            ExprKind::Proposition(_) | ExprKind::Forall { .. } | ExprKind::Exists { .. } => true,
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => match operator {
                BinaryOp::Implies => true,
                BinaryOp::And | BinaryOp::Or => {
                    self.reads_as_prop(left) || self.reads_as_prop(right)
                }
                _ => false,
            },
            ExprKind::Name(name) => match self.lookup(&name.text) {
                Some(local) => same_type(&local.ty, &Type::Prop),
                None => matches!(self.globals.get(&name.text), Some(Global::Prop(_))),
            },
            ExprKind::Call { callee, .. } => match &callee.kind {
                ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                    match self.globals.get(&name.text) {
                        Some(Global::Fn(info)) => same_type(&info.result, &Type::Prop),
                        Some(Global::Prop(_)) => true,
                        _ => false,
                    }
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn tuple(&mut self, items: &[ast::Expr], expected: Option<&Type>, span: Span) -> Elab<Value> {
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

    /// Checks arguments against a telescope of parameter types, written over
    /// the identities `ids`: each argument's term replaces its parameter in
    /// the types that follow. On return `tys` no longer mentions `ids`.
    pub fn arguments(
        &mut self,
        arguments: &[ast::Expr],
        ids: &[VarId],
        tys: &mut [Type],
        what: &str,
        span: Span,
    ) -> Elab<Vec<Expr>> {
        if arguments.len() != ids.len() {
            let message = format!(
                "{what} takes {} value{}, and {} {} given",
                ids.len(),
                if ids.len() == 1 { "" } else { "s" },
                arguments.len(),
                if arguments.len() == 1 { "was" } else { "were" },
            );
            return self.fail("L0208", message, span);
        }
        let mut exprs = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let value = self.check(argument, &tys[index].clone())?;
            let term = self.term(&value, argument.span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(ids[index], &term);
            }
            // The last entry may be a result type.
            exprs.push(value.expr);
        }
        Ok(exprs)
    }

    fn struct_literal(
        &mut self,
        name: &ast::Name,
        fields: &[ast::ValueField],
        span: Span,
    ) -> Elab<Value> {
        let Some(Global::Struct(info)) = self.globals.get(&name.text).cloned() else {
            if self.failed.contains(&name.text) {
                return Err(());
            }
            return self.fail(
                "L0204",
                format!("unknown struct `{}`", name.text),
                name.span,
            );
        };
        let mut given: Vec<(String, &ast::Expr, Span)> = Vec::new();
        for field in fields {
            let field_name = match (&field.name, &field.value.kind) {
                (Some(name), _) => name.text.clone(),
                (None, ExprKind::Name(name)) => name.text.clone(),
                (None, _) => {
                    return self.fail("L0224", "a field needs a name: `name: value`", field.span);
                }
            };
            if given.iter().any(|(earlier, _, _)| *earlier == field_name) {
                return self.fail(
                    "L0224",
                    format!("field `{field_name}` is given twice"),
                    field.span,
                );
            }
            if !info
                .fields
                .iter()
                .any(|declared| declared.name == field_name)
            {
                let message = format!("`{}` has no field `{field_name}`", info.name);
                return self.fail("L0224", message, field.span);
            }
            given.push((field_name, &field.value, field.span));
        }
        let mut exprs = Vec::new();
        let mut tys: Vec<Type> = info.fields.iter().map(|field| field.ty.clone()).collect();
        for (index, declared) in info.fields.iter().enumerate() {
            let Some((_, value, _)) = given.iter().find(|(name, _, _)| *name == declared.name)
            else {
                let message = format!("missing field `{}` of `{}`", declared.name, info.name);
                return self.fail("L0224", message, span);
            };
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

    fn enum_variant(&mut self, path: &ast::Path) -> Elab<Option<(std::rc::Rc<EnumInfo>, usize)>> {
        match self.globals.get(&path.prefix.text).cloned() {
            Some(Global::Enum(info)) => {
                match info
                    .variants
                    .iter()
                    .position(|(name, _)| *name == path.name.text)
                {
                    Some(index) => Ok(Some((info, index))),
                    None => {
                        let message =
                            format!("`{}` has no variant `{}`", info.name, path.name.text);
                        self.fail("L0212", message, path.name.span)
                    }
                }
            }
            Some(Global::Prop(_)) => Ok(None),
            Some(_) => self.fail(
                "L0212",
                format!("`{}` is not an enum or a proposition", path.prefix.text),
                path.prefix.span,
            ),
            None if self.failed.contains(&path.prefix.text) => Err(()),
            None => self.fail(
                "L0204",
                format!("unknown name `{}`", path.prefix.text),
                path.prefix.span,
            ),
        }
    }

    fn variant(
        &mut self,
        path: &ast::Path,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let Some((info, index)) = self.enum_variant(path)? else {
            let Some(Global::Prop(info)) = self.globals.get(&path.prefix.text).cloned() else {
                unreachable!("`enum_variant` saw a proposition")
            };
            return self.construct(&info, path, arguments, expected, span);
        };
        let payload = &info.variants[index].1;
        let ids: Vec<VarId> = payload.iter().map(|binder| binder.id).collect();
        let mut tys: Vec<Type> = payload.iter().map(|binder| binder.ty.clone()).collect();
        let what = format!("`{}::{}`", info.name, path.name.text);
        let payload = self.arguments(arguments, &ids, &mut tys, &what, span)?;
        Ok(Value::new(
            Expr::Variant {
                id: info.id,
                enum_name: info.name.clone(),
                index,
                variant_name: path.name.text.clone(),
                payload,
            },
            Type::Enum(info.id),
        ))
    }

    fn call(
        &mut self,
        callee: &ast::Expr,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        match &callee.kind {
            ExprKind::Path(path) => self.variant(path, arguments, expected, span),
            ExprKind::Member { value, name } => self.method(value, name, arguments, span),
            ExprKind::Name(name) if self.lookup(&name.text).is_none() => {
                match self.globals.get(&name.text).cloned() {
                    Some(Global::Fn(info)) => self.call_fn(&info, arguments, span),
                    Some(Global::Prop(info)) => {
                        let term = self.prop_application(&info, arguments, span)?;
                        Ok(Value::new(Expr::Prop(term), Type::Prop))
                    }
                    Some(_) => self.fail(
                        "L0207",
                        format!("`{}` cannot be called", name.text),
                        name.span,
                    ),
                    None if self.failed.contains(&name.text) => Err(()),
                    None if matches!(name.text.as_str(), "rewrite" | "unfold" | "fold") => {
                        self.proof_form(&name.text, arguments, expected, span)
                    }
                    None => self.fail(
                        "L0204",
                        format!("unknown function `{}`", name.text),
                        name.span,
                    ),
                }
            }
            _ => {
                let callee = self.infer(callee)?;
                self.apply(callee, arguments, span)
            }
        }
    }

    fn call_fn(&mut self, info: &FnInfo, arguments: &[ast::Expr], span: Span) -> Elab<Value> {
        let ids: Vec<VarId> = info.params.iter().map(|param| param.id).collect();
        let mut tys: Vec<Type> = info.params.iter().map(|param| param.ty.clone()).collect();
        tys.push(info.result.clone());
        let what = format!("`{}`", info.name);
        // The result type rides along so that it is instantiated too.
        let count = ids.len();
        if arguments.len() != count {
            let mut only_params = tys[..count].to_vec();
            return self
                .arguments(arguments, &ids, &mut only_params, &what, span)
                .map(|_| unreachable!("the counts differ"));
        }
        let mut exprs = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let value = self.check(argument, &tys[index].clone())?;
            let term = self.term(&value, argument.span)?;
            for later in tys[index + 1..].iter_mut() {
                *later = later.replace_var(ids[index], &term);
            }
            exprs.push(value.expr);
        }
        let ty = tys.pop().expect("the result type was pushed");
        match info.reference {
            FnRef::Math(id) => Ok(Value::new(
                Expr::CallMath {
                    id,
                    name: info.name.clone(),
                    arguments: exprs,
                    ty: ty.clone(),
                },
                ty,
            )),
            FnRef::Exec(id) => {
                if self.total {
                    self.diagnostics.push(
                        crate::diagnostic::Diagnostic::error(
                            "L0209",
                            format!("`{}` is an ordinary `fn` and cannot be called here", info.name),
                            span,
                        )
                        .note("an ordinary `fn` may fail to return, so a `math fn` and a proposition can only call a `math fn`"),
                    );
                    return Err(());
                }
                let result = VarId::fresh();
                self.declare_result(result, &ty, span)?;
                Ok(Value::new(
                    Expr::CallFn {
                        id,
                        name: info.name.clone(),
                        arguments: exprs,
                        result,
                        ty: ty.clone(),
                    },
                    ty,
                ))
            }
        }
    }

    pub fn prop_application(
        &mut self,
        info: &PropInfo,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Term> {
        if arguments.len() != info.params.len() {
            let message = format!(
                "`{}` takes {} argument(s), and {} were given",
                info.name,
                info.params.len(),
                arguments.len()
            );
            return self.fail("L0208", message, span);
        }
        let mut terms = Vec::new();
        for (argument, param) in arguments.iter().zip(&info.params) {
            let value = self.check(argument, &param.ty)?;
            terms.push(self.term(&value, argument.span)?);
        }
        Ok(Term::PropApp(info.id, terms))
    }

    fn method(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Name,
        arguments: &[ast::Expr],
        span: Span,
    ) -> Elab<Value> {
        let prim = match name.text.as_str() {
            "wrapping_add" => Prim::WrappingAdd,
            "wrapping_sub" => Prim::WrappingSub,
            other => {
                return self.fail("L0207", format!("unknown method `{other}`"), name.span);
            }
        };
        let receiver = self.check(receiver, &Type::U8)?;
        if arguments.len() != 1 {
            return self.fail("L0208", format!("`{}` takes one argument", name.text), span);
        }
        let argument = self.check(&arguments[0], &Type::U8)?;
        Ok(Value::new(
            Expr::Method {
                prim,
                receiver: Box::new(receiver.expr),
                arguments: vec![argument.expr],
            },
            Type::U8,
        ))
    }

    // --- Branching ----------------------------------------------------------------

    pub fn branch(
        &mut self,
        branch: &Branch<'_>,
        expected: Option<&Type>,
    ) -> Elab<(typed::Block, Type, bool)> {
        match branch {
            Branch::Block(block) => self.block(block, expected),
            Branch::Expr(ast::Expr {
                kind: ExprKind::Block(block),
                ..
            }) => self.block(block, expected),
            Branch::Expr(expr) => {
                let value = match expected {
                    Some(expected) => self.check(expr, expected)?,
                    None => self.infer(expr)?,
                };
                Ok((
                    typed::Block {
                        stmts: Vec::new(),
                        tail: Some(Box::new(value.expr)),
                    },
                    value.ty,
                    value.never,
                ))
            }
        }
    }

    fn conditional(
        &mut self,
        condition: &ast::Expr,
        then: Branch<'_>,
        otherwise: Branch<'_>,
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let condition_value = self.check(condition, &Type::Bool)?;
        // Branch facts speak of the comparison performed: `a != b` tests
        // `a == b` and exchanges the branches.
        let (tested, negated) = match &condition_value.expr {
            Expr::Compare {
                op: CompareOp::Ne,
                left,
                right,
            } => (
                Expr::Compare {
                    op: CompareOp::Eq,
                    left: left.clone(),
                    right: right.clone(),
                },
                true,
            ),
            other => (other.clone(), false),
        };
        let tested = self.term(&Value::new(tested, Type::Bool), condition.span)?;
        let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
        let fact = |holds: bool| Term::eq(Type::Bool, tested.clone(), Term::Bool(holds != negated));

        let mark = self.mark();
        let then_result = self
            .assume(then_fact, fact(true), condition.span)
            .and_then(|()| self.branch(&then, expected));
        self.close(mark);
        let (then_block, then_ty, then_never) = then_result?;

        let expected_else = match expected {
            Some(expected) => Some(expected.clone()),
            None if !then_never => Some(then_ty.clone()),
            None => None,
        };
        let mark = self.mark();
        let else_result = self
            .assume(else_fact, fact(false), condition.span)
            .and_then(|()| self.branch(&otherwise, expected_else.as_ref()));
        self.close(mark);
        let (else_block, else_ty, else_never) = else_result?;

        let never = then_never && else_never;
        let ty = match expected {
            Some(expected) => expected.clone(),
            None if !then_never => then_ty,
            None => else_ty,
        };
        let result = VarId::fresh();
        let expr = Expr::If {
            condition: Box::new(condition_value.expr),
            then_fact,
            else_fact,
            then_block,
            else_block,
            ty: ty.clone(),
            result,
        };
        if !never && !is_pure(&expr) {
            self.declare_result(result, &ty, span)?;
        }
        Ok(Value { expr, ty, never })
    }

    fn match_(
        &mut self,
        scrutinee: &ast::Expr,
        arms: &[ast::MatchArm],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        let scrutinee_value = self.infer(scrutinee)?;
        if matches!(scrutinee_value.ty, Type::Proof(_)) {
            return self.match_evidence(scrutinee_value, scrutinee.span, arms, expected, span);
        }
        let Type::Enum(id) = &scrutinee_value.ty else {
            let shown = self.show_type(&scrutinee_value.ty);
            let message = format!("`match` takes apart an enum or evidence, and this is `{shown}`");
            return self.fail("L0212", message, scrutinee.span);
        };
        let info = self.enum_by_id(*id).expect("an enum type was declared");
        let scrutinee_term = self.term(&scrutinee_value, scrutinee.span)?;
        let payload_types =
            case_variants(&self.ctx, &scrutinee_value.ty).expect("an enum has variants");

        // Which source arm handles each variant.
        let mut chosen: Vec<Option<&ast::MatchArm>> = vec![None; info.variants.len()];
        for arm in arms {
            match &arm.pattern.kind {
                PatternKind::Wildcard => {
                    if chosen.iter().all(Option::is_some) {
                        return self.fail("L0214", "this arm is never reached", arm.pattern.span);
                    }
                    for slot in chosen.iter_mut().filter(|slot| slot.is_none()) {
                        *slot = Some(arm);
                    }
                }
                PatternKind::Variant { path, .. } => {
                    if path.prefix.text != info.name {
                        let message = format!(
                            "this arm is for `{}`, and the value matched is a `{}`",
                            path.prefix.text, info.name
                        );
                        return self.fail("L0212", message, path.span);
                    }
                    let Some((_, index)) = self.enum_variant(path)? else {
                        return Err(());
                    };
                    if chosen[index].is_some() {
                        return self.fail("L0214", "this arm is never reached", arm.pattern.span);
                    }
                    chosen[index] = Some(arm);
                }
                _ => {
                    return self.fail(
                        "L0290",
                        "an arm's pattern is `Enum::Variant(names...)` or `_`; other patterns are not supported yet",
                        arm.pattern.span,
                    );
                }
            }
        }
        let missing: Vec<String> = chosen
            .iter()
            .zip(&info.variants)
            .filter(|(arm, _)| arm.is_none())
            .map(|(_, (name, _))| format!("`{}::{name}`", info.name))
            .collect();
        if !missing.is_empty() {
            return self.fail(
                "L0213",
                format!("no arm handles {}", missing.join(", ")),
                span,
            );
        }

        let mut typed_arms = Vec::new();
        let mut ty: Option<Type> = expected.cloned();
        let mut never = true;
        for (index, arm) in chosen.iter().enumerate() {
            let arm = arm.expect("every variant has an arm");
            let (variant_name, declared) = &info.variants[index];
            let mark = self.mark();
            let arm_result = (|| {
                let names: Vec<Option<&ast::Name>> = match &arm.pattern.kind {
                    PatternKind::Variant { arguments, path } => {
                        let given = arguments.as_deref().unwrap_or(&[]);
                        if given.len() != declared.len() {
                            let message = format!(
                                "`{}::{variant_name}` carries {} value(s), and the pattern names {}",
                                info.name,
                                declared.len(),
                                given.len()
                            );
                            return self.fail("L0208", message, path.span);
                        }
                        let mut names = Vec::new();
                        for pattern in given {
                            match &pattern.kind {
                                PatternKind::Name(name) => names.push(Some(name)),
                                PatternKind::Wildcard => names.push(None),
                                _ => {
                                    return self.fail(
                                        "L0290",
                                        "patterns inside a variant are names or `_` for now",
                                        pattern.span,
                                    );
                                }
                            }
                        }
                        names
                    }
                    _ => vec![None; declared.len()],
                };
                let mut payload: Vec<Binder> = Vec::new();
                let telescope = Type::Tuple(payload_types[index].clone());
                for (field, name) in names.iter().enumerate() {
                    let earlier: Vec<Term> = payload.iter().map(Binder::term).collect();
                    let field_ty = telescope_entry(&telescope, field, &earlier)
                        .expect("the payload has this field");
                    let binder = Binder {
                        id: VarId::fresh(),
                        name: name.map_or_else(|| "_".to_string(), |name| name.text.clone()),
                        ty: field_ty,
                    };
                    let declared = self.ctx.declare_with(binder.id, binder.ty.clone(), false);
                    self.kernel(declared, arm.pattern.span)?;
                    if name.is_some() {
                        self.bind(&binder.name, binder.id, &binder.ty);
                    }
                    payload.push(binder);
                }
                let ids: Vec<VarId> = payload.iter().map(|binder| binder.id).collect();
                let fact = HypId::fresh();
                let claim = Term::eq(
                    scrutinee_value.ty.clone(),
                    scrutinee_term.clone(),
                    variant_term(&scrutinee_value.ty, index, &ids, &payload_types[index]),
                );
                self.assume(fact, claim, arm.pattern.span)?;
                let (body, body_ty, body_never) =
                    self.branch(&Branch::Expr(&arm.body), ty.as_ref())?;
                Ok((payload, fact, body, body_ty, body_never))
            })();
            self.close(mark);
            let (payload, fact, body, body_ty, body_never) = arm_result?;
            if !body_never {
                never = false;
                ty.get_or_insert(body_ty);
            }
            typed_arms.push(MatchArm {
                variant_name: variant_name.clone(),
                payload,
                fact,
                body,
            });
        }
        let ty = ty.unwrap_or_else(unit_type);
        let result = VarId::fresh();
        let expr = Expr::Match {
            scrutinee: Box::new(scrutinee_value.expr),
            enum_name: info.name.clone(),
            arms: typed_arms,
            ty: ty.clone(),
            result,
        };
        if !never && !is_pure(&expr) {
            self.declare_result(result, &ty, span)?;
        }
        Ok(Value { expr, ty, never })
    }

    // --- Loops --------------------------------------------------------------------

    /// The state binders of a loop, whose types may mention `index` and the
    /// state before them, and the initial values, which may not.
    fn loop_state(
        &mut self,
        state: &[ast::StateParameter],
        index: Option<(&Binder, &Term)>,
    ) -> Elab<Vec<(Binder, Expr)>> {
        let mark = self.mark();
        let binders = (|| {
            if let Some((index, _)) = index {
                self.declare(
                    index,
                    false,
                    state
                        .first()
                        .map_or(Span::new(self.source.id, 0, 0), |s| s.span),
                )?;
            }
            self.telescope(
                state
                    .iter()
                    .map(|parameter| (Some(&parameter.name), &parameter.ty, parameter.span)),
            )
        })();
        self.close(mark);
        let binders = binders?;

        let mut tys: Vec<Type> = binders
            .iter()
            .map(|binder| match index {
                Some((index, start)) => binder.ty.replace_var(index.id, start),
                None => binder.ty.clone(),
            })
            .collect();
        let mut result = Vec::new();
        for (position, parameter) in state.iter().enumerate() {
            let value = self.check(&parameter.initial, &tys[position].clone())?;
            let term = self.term(&value, parameter.initial.span)?;
            for later in tys[position + 1..].iter_mut() {
                *later = later.replace_var(binders[position].id, &term);
            }
            result.push((binders[position].clone(), value.expr));
        }
        Ok(result)
    }

    fn loop_body(&mut self, body: &ast::Block, what: &str) -> Elab<typed::Block> {
        let (block, _, never) = self.block(body, None)?;
        if !never {
            let message = format!("every path through {what} must end in `continue(...)`");
            let message = if what.contains("loop") {
                message.replace("`continue(...)`", "`continue(...)` or `break`")
            } else {
                message
            };
            return self.fail("L0216", message, body.span);
        }
        Ok(block)
    }

    fn loop_(
        &mut self,
        state: &[ast::StateParameter],
        result: &ast::Type,
        body: &ast::Block,
        span: Span,
    ) -> Elab<Value> {
        if self.total {
            self.diagnostics.push(
                crate::diagnostic::Diagnostic::error("L0215", "`loop` cannot appear here", span)
                    .note("a `loop` may run forever, and a `math fn` must return; a bounded `for` always does"),
            );
            return Err(());
        }
        let state = self.loop_state(state, None)?;
        let result_ty = self.ty(result)?;
        let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();

        let mark = self.mark();
        let body_result = (|| {
            for binder in &binders {
                self.declare(binder, false, span)?;
            }
            self.loops.push(LoopTarget {
                state: binders.clone(),
                advance: None,
                result: Some(result_ty.clone()),
            });
            let block = self.loop_body(body, "a loop");
            self.loops.pop();
            block
        })();
        self.close(mark);
        let body = body_result?;

        let result = VarId::fresh();
        self.declare_result(result, &result_ty, span)?;
        Ok(Value::new(
            Expr::Loop {
                state,
                result_ty: result_ty.clone(),
                body,
                result,
            },
            result_ty,
        ))
    }

    fn for_(
        &mut self,
        index: &ast::Name,
        lower: &ast::Expr,
        upper: &ast::Expr,
        state: &[ast::StateParameter],
        body: &ast::Block,
        span: Span,
    ) -> Elab<Value> {
        let lo = self.check(lower, &Type::U8)?;
        let hi = self.check(upper, &Type::U8)?;
        let lo_term = self.term(&lo, lower.span)?;
        let hi_term = self.term(&hi, upper.span)?;
        let prelude = self.prelude;
        let ordered_claim = prelude.u8_le_prop(lo_term.clone(), hi_term.clone());
        let ordered = self.solve(&ordered_claim, lower.span.through(upper.span), None)?;

        let index = Binder {
            id: VarId::fresh(),
            name: index.text.clone(),
            ty: Type::U8,
        };
        let state = self.loop_state(state, Some((&index, &lo_term)))?;
        let binders: Vec<Binder> = state.iter().map(|(binder, _)| binder.clone()).collect();
        let (lower_fact, upper_fact) = (HypId::fresh(), HypId::fresh());

        let mark = self.mark();
        let body_result = (|| {
            self.declare(&index, false, span)?;
            for binder in &binders {
                self.declare(binder, false, span)?;
            }
            self.assume(
                lower_fact,
                prelude.u8_le_prop(lo_term.clone(), index.term()),
                span,
            )?;
            self.assume(
                upper_fact,
                prelude.u8_lt_prop(index.term(), hi_term.clone()),
                span,
            )?;
            self.loops.push(LoopTarget {
                state: binders.clone(),
                advance: Some((index.id, Term::wrapping_add(index.term(), Term::U8(1)))),
                result: None,
            });
            let block = self.loop_body(body, "the body of a `for`");
            self.loops.pop();
            block
        })();
        self.close(mark);
        let body = body_result?;

        // The final state: the telescope at the upper bound.
        let ty = tuple_over(&binders).replace_var(index.id, &hi_term);
        let result = VarId::fresh();
        let expr = Expr::For {
            index,
            lower: lower_fact,
            upper: upper_fact,
            lo: Box::new(lo.expr),
            hi: Box::new(hi.expr),
            ordered,
            state,
            body,
            result,
        };
        if !is_pure(&expr) {
            self.declare_result(result, &ty, span)?;
        }
        Ok(Value::new(expr, ty))
    }

    // --- Blocks -------------------------------------------------------------------

    /// Elaborates a block. The caller opens and closes the scope.
    pub fn block(
        &mut self,
        block: &ast::Block,
        expected: Option<&Type>,
    ) -> Elab<(typed::Block, Type, bool)> {
        let mut stmts = Vec::new();
        let mut failed = false;
        let mut never = false;
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Error => failed = true,
                StatementKind::Let {
                    pattern,
                    annotation,
                    value,
                } => {
                    let result = (|| {
                        let value = match annotation {
                            Some(annotation) => {
                                let ty = self.ty(annotation)?;
                                self.check(value, &ty)?
                            }
                            None => self.infer(value)?,
                        };
                        let term = self.term(&value, statement.span)?;
                        let pattern = self.bind_pattern(pattern, term)?;
                        Ok(Stmt::Let {
                            pattern,
                            value: value.expr,
                        })
                    })();
                    match result {
                        Ok(stmt) => stmts.push(stmt),
                        Err(()) => {
                            self.poison(pattern);
                            failed = true;
                        }
                    }
                }
                StatementKind::Expression(expr) => match self.infer(expr) {
                    Ok(value) => {
                        never |= value.never;
                        stmts.push(Stmt::Expr(value.expr));
                    }
                    Err(()) => failed = true,
                },
            }
        }
        if failed {
            // What follows may depend on a binding that was not made.
            return Err(());
        }
        match block.tail.as_deref() {
            Some(tail) => {
                let value = match expected {
                    Some(expected) => self.check(tail, expected)?,
                    None => self.infer(tail)?,
                };
                Ok((
                    typed::Block {
                        stmts,
                        tail: Some(Box::new(value.expr)),
                    },
                    value.ty,
                    value.never || never,
                ))
            }
            None if never => {
                // `continue(next);` written as a statement ends the block.
                let tail = match stmts.pop() {
                    Some(Stmt::Expr(expr)) => expr,
                    other => {
                        stmts.extend(other);
                        return self.fail(
                            "L0216",
                            "statements follow a transfer of control",
                            block.span,
                        );
                    }
                };
                Ok((
                    typed::Block {
                        stmts,
                        tail: Some(Box::new(tail)),
                    },
                    unit_type(),
                    true,
                ))
            }
            None => {
                let ty = unit_type();
                if let Some(expected) = expected
                    && !same_type(expected, &ty)
                {
                    let shown = self.show_type(expected);
                    return self.fail(
                        "L0220",
                        format!("this block ends without a value, and `{shown}` is expected"),
                        block.span,
                    );
                }
                Ok((typed::Block { stmts, tail: None }, ty, false))
            }
        }
    }

    /// Binds a `let` pattern to a value, as the checker will: a name is
    /// declared with the equation `name == value`, and a tuple pattern binds
    /// each part to a projection.
    fn bind_pattern(&mut self, pattern: &ast::Pattern, value: Term) -> Elab<Pattern> {
        match &pattern.kind {
            PatternKind::Wildcard => Ok(Pattern::Wildcard),
            PatternKind::Group(inner) => self.bind_pattern(inner, value),
            PatternKind::Name(name) => {
                let (id, equation) = (VarId::fresh(), HypId::fresh());
                let defined = self.ctx.define_with(id, equation, &value);
                let ty = self.kernel(defined, pattern.span)?;
                if !matches!(ty, Type::Proof(_)) {
                    self.facts.push(super::env::Fact {
                        proof: Proof::hyp(equation),
                        claim: Term::eq(ty.clone(), Term::var(id), value),
                    });
                }
                self.bind(&name.text, id, &ty);
                Ok(Pattern::Bind {
                    binder: Binder {
                        id,
                        name: name.text.clone(),
                        ty,
                    },
                    equation,
                })
            }
            PatternKind::Unit => Ok(Pattern::Tuple(Vec::new())),
            PatternKind::Tuple(patterns) => {
                let ty = self.type_of(&value, pattern.span)?;
                match &ty {
                    Type::Tuple(fields) if fields.len() == patterns.len() => {}
                    _ => {
                        let shown = self.show_type(&ty);
                        let message = format!(
                            "this pattern has {} parts, and the value is `{shown}`",
                            patterns.len()
                        );
                        return self.fail("L0220", message, pattern.span);
                    }
                }
                let mut parts = Vec::new();
                for (index, part) in patterns.iter().enumerate() {
                    parts.push(self.bind_pattern(part, Term::proj(value.clone(), index))?);
                }
                Ok(Pattern::Tuple(parts))
            }
            _ => self.fail(
                "L0290",
                "a `let` pattern is a name, `_`, or a tuple of those for now",
                pattern.span,
            ),
        }
    }
}

pub(super) enum Branch<'a> {
    Block(&'a ast::Block),
    Expr(&'a ast::Expr),
}
