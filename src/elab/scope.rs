//! Result types cross lexical boundaries only when their dependencies do.
//! Captured proposition values remain ordinary logical values of type Prop;
//! this check concerns free names exposed by the result's *type*.

use std::collections::HashSet;

use crate::diagnostic::Diagnostic;
use crate::kernel::{Binding, Type, VarId};
use crate::source::Span;
use crate::typed;

use super::env::{Elab, Env};
use super::explain::free_variables;
use super::exprs::Value;

pub(super) struct ResultScope {
    available: HashSet<VarId>,
    mutable: HashSet<VarId>,
}

fn type_variables(ty: &Type, result: &mut HashSet<VarId>) {
    match ty {
        Type::Instance(base, args) => {
            type_variables(base, result);
            for argument in args {
                result.extend(free_variables(argument));
            }
        }
        Type::Boxed(inner) | Type::Buffer(inner) => type_variables(inner, result),
        Type::Proof(claim) => result.extend(free_variables(claim)),
        Type::Tuple(fields) => {
            for field in fields {
                type_variables(field, result);
            }
        }
        Type::Fn(parameters, output) => {
            for parameter in parameters {
                type_variables(parameter, result);
            }
            type_variables(output, result);
        }
        _ => {}
    }
}

impl Env<'_> {
    /// Capture before entering the block/arm, before introducing its locals.
    pub(super) fn result_scope(&self) -> ResultScope {
        ResultScope {
            available: self
                .ctx
                .bindings()
                .filter_map(|binding| match binding {
                    Binding::Var { id, .. } => Some(id),
                    Binding::Hyp { .. } => None,
                })
                .collect(),
            mutable: self
                .names
                .iter()
                .filter_map(|local| local.binding)
                .collect(),
        }
    }

    /// Run before closing names or restoring mutation versions. A write to
    /// an already-visible mutable binding exposes its new version to the
    /// receiving block; a fresh local, or an opaque call result, does not.
    pub(super) fn check_scope_result(
        &mut self,
        scope: &ResultScope,
        result: Elab<(typed::Block, Type, bool)>,
        span: Span,
    ) -> Elab<(typed::Block, Type, bool)> {
        let (mut block, mut ty, never) = result?;
        if never {
            return Ok((block, ty, never));
        }
        let mut available = scope.available.clone();
        available.extend(self.names.iter().filter_map(|local| {
            local
                .binding
                .filter(|binding| scope.mutable.contains(binding))
                .map(|_| local.id)
        }));
        let escapes = |ty: &Type| {
            let mut free = HashSet::new();
            type_variables(ty, &mut free);
            free.retain(|id| !available.contains(id));
            free
        };
        if escapes(&ty).is_empty() {
            return Ok((block, ty, never));
        }

        // A justified definition can remove a local from a proof's exposed
        // claim. Coercion builds and checks the required proof transports;
        // changing a type here without transporting evidence would be wrong.
        if let Type::Proof(claim) = &ty {
            let known = self.knowledge();
            let (normal, _) = self.normalize(claim, &known.definitions);
            let normalized = Type::proof(normal);
            if escapes(&normalized).is_empty()
                && let Some(tail) = block.tail.take()
            {
                let value = self.coerce(Value::new(*tail, ty), &normalized, span)?;
                block.tail = Some(Box::new(value.expr));
                ty = value.ty;
                return Ok((block, ty, never));
            }
        }

        let mut names: Vec<_> = escapes(&ty)
            .iter()
            .map(|id| {
                self.labels
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| "an unnamed local result".into())
            })
            .collect();
        names.sort();
        names.dedup();
        let shown = self.show_type(&ty);
        self.diagnostics.push(Diagnostic::error(
            "L0280",
            format!("result type `{shown}` exposes a local value outside its scope"),
            span,
        ).note(format!("not available to the receiver: {}", names.join(", ")))
         .note("return a struct or tuple carrying the value or proposition alongside its evidence, or prove a claim about values already available outside this scope")
         .note("an ordinary call's result is not replaced by an invented equation for the function's body"));
        Err(())
    }
}
