//! The built-in forms, `name!(...)`: `prop!` states a proposition, `prove!`
//! states a claim where it stands and finds its evidence, `rewrite!`,
//! `unfold!` and `fold!` are the explicit equality steps, and the rest are
//! not in Locus yet and say which task brings them.

use crate::ast::{self, Form};
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::kernel::{Proof, Term, Type};
use crate::source::Span;
use crate::typed::Expr;

use super::env::{Elab, Env, Fact};
use super::exprs::Value;

impl Env<'_> {
    pub(super) fn form(
        &mut self,
        form: Form,
        name_span: Span,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        span: Span,
    ) -> Elab<Value> {
        match form {
            Form::Prop => {
                let term = self.formula(&arguments[0])?;
                Ok(Value::new(Expr::Prop(term), Type::Prop))
            }
            Form::Prove => {
                let (proof, claim) = self.prove_claim(&arguments[0], span)?;
                Ok(Value::new(Expr::Proof(proof), Type::proof(claim)))
            }
            Form::Rewrite | Form::Unfold | Form::Fold => {
                self.proof_form(form, arguments, expected, span)
            }
            _ => self.form_not_yet(form, name_span),
        }
    }

    /// `prove!(claim)`: the claim as a proposition, and evidence of it found
    /// as a `_` would find it. A failure is reported here.
    fn prove_claim(&mut self, formula: &ast::Expr, span: Span) -> Elab<(Proof, Term)> {
        let claim = self.formula(formula)?;
        let proof = self.solve(&claim, span, None)?;
        Ok((proof, claim))
    }

    /// `prove!(claim);` as a statement: the claim is proved, and stays known
    /// for what follows. It erases to nothing, so no statement is kept.
    pub(super) fn prove_statement(&mut self, formula: &ast::Expr, span: Span) -> Elab<()> {
        let (proof, claim) = self.prove_claim(formula, span)?;
        self.facts.push(Fact { proof, claim });
        Ok(())
    }

    /// The forms that have a spelling and no meaning yet, each with the
    /// task that gives it one.
    #[inline(never)]
    fn form_not_yet<T>(&mut self, form: Form, span: Span) -> Elab<T> {
        let arrives = match form {
            Form::Old => "references as parameters (O3, LOC-184)",
            Form::Snapshot => "logic-only types (E8, LOC-174)",
            Form::Recurse => "recursion (LOC-53)",
            Form::Assert
            | Form::Unreachable
            | Form::Todo
            | Form::Panic
            | Form::DebugAssert
            | Form::Matches => "the forms that panic (E10, LOC-190)",
            _ => "`Vec` (LOC-83)",
        };
        self.diagnostics.push(
            Diagnostic::error(
                "L0290",
                format!("`{}!` is not in Locus yet", form.name()),
                span,
            )
            .note(format!("it arrives with {arrives}")),
        );
        Err(())
    }

    /// `rewrite(...)`, `unfold(...)`, or `fold(...)` written as a call: the
    /// forms are spelled with `!` now, and the bare names are free.
    #[inline(never)]
    pub(super) fn retired_bare_form<T>(&mut self, name: &ast::Name) -> Elab<T> {
        let span = name.span.at_end();
        self.diagnostics.push(
            Diagnostic::error(
                "L0231",
                format!("`{0}` is a built-in form, written `{0}!(...)`", name.text),
                name.span,
            )
            .note("the forms take `!` as a Rust macro does; a plain `rewrite`, `unfold`, or `fold` is an ordinary name, and none is defined here")
            .suggest(Suggestion {
                message: format!("write `{}!`", name.text),
                span,
                replacement: "!".into(),
                applicability: Applicability::MachineApplicable,
            }),
        );
        Err(())
    }
}
