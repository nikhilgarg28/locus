//! The built-in forms, `name!(...)`: `prop!` states a proposition, `prove!`
//! states a claim where it stands and finds its evidence, `rewrite!`,
//! `unfold!` and `fold!` are the explicit equality steps, `snapshot!`
//! builds a `Ghost<T>` (E8), and the rest are not in Locus yet and say
//! which task brings them.
//!
//! The forms that mean what they mean in Rust and panic: `panic!`, `todo!`,
//! and `unreachable!` yield no value and stand where any type is expected;
//! `assert!` and `debug_assert!` are checks of type `()`. Each lowers to
//! the panic ending of the check IR with the message Rust's form prints,
//! and a message is a string literal taken as it is written: Rust's format
//! arguments are not in Locus yet. Under `no_panic` the checker demands
//! evidence of `False` at every panic ending, and the elaborator finds it
//! as it fills a hole: `unreachable!()` asks for `False` where it stands,
//! and `assert!(c)` asks for `c`, which contradicts the arm where the check
//! fails. Without the promise the same evidence is tried and attached when
//! found, and the form panics as Rust's does otherwise. After `assert!(c)`
//! the condition is a fact, the value of the check's statement; a
//! `debug_assert!` teaches nothing, since a release build skips it.

use crate::ast::{self, ExprKind, Form};
use crate::diagnostic::{Applicability, Diagnostic, Suggestion};
use crate::kernel::{Axiom, CmpOp, HypId, MachineInt, Proof, Term, Type, VarId, check_proof};
use crate::source::Span;
use crate::typed::{Expr, PanicForm};

use super::env::{Elab, Env, Fact};
use super::exprs::{Value, unit_type};
use super::solve::Test;

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
            Form::Panic | Form::Todo | Form::Unreachable => {
                self.panic_form(form, arguments, expected, name_span, span)
            }
            Form::Assert | Form::DebugAssert => self.assert_form(form, arguments, name_span, span),
            Form::Snapshot => self.snapshot(arguments, expected, name_span, span),
            _ => self.form_not_yet(form, name_span),
        }
    }

    // --- The forms that panic ----------------------------------------------------------

    /// `panic!`, `todo!`, or `unreachable!`, with a message or without: a
    /// panic ending, of whatever type is expected here.
    fn panic_form(
        &mut self,
        form: Form,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        name_span: Span,
        span: Span,
    ) -> Elab<Value> {
        self.runs_here(form, name_span)?;
        let argument = self.message_argument(form, arguments, 0)?;
        let which = match form {
            Form::Panic => PanicForm::Panic,
            Form::Todo => PanicForm::Todo,
            _ => PanicForm::Unreachable,
        };
        let unreachable = match which {
            PanicForm::Unreachable => self.unreachable_evidence(span)?,
            _ if self.promises.no_panic => return self.panics_under_no_panic(form, name_span),
            _ => None,
        };
        let ty = expected.cloned().unwrap_or_else(unit_type);
        let result = VarId::fresh();
        self.declare_result(result, &ty, span)?;
        Ok(Value {
            expr: Expr::Panic {
                form: which,
                argument,
                unreachable,
                ty: ty.clone(),
                result,
            },
            ty,
            never: true,
        })
    }

    /// Evidence that this point is never reached, `False` as a hole is
    /// filled: required under `no_panic`, and tried otherwise.
    fn unreachable_evidence(&mut self, span: Span) -> Elab<Option<Proof>> {
        let falsehood = self.prelude.falsehood_prop();
        if !self.promises.no_panic {
            return Ok(self.try_solve(&falsehood, span));
        }
        match self.solve(&falsehood, span, None) {
            Ok(proof) => Ok(Some(proof)),
            Err(()) => {
                let item = self.item_name.clone();
                self.reword_unsolved(format!(
                    "`unreachable!()` needs evidence that this point is unreachable in `{item}`, which promises no_panic"
                ));
                Err(())
            }
        }
    }

    /// `assert!(c)`, `assert!(c, "message")`, and the same for
    /// `debug_assert!`: a check whose failure is a panic ending.
    fn assert_form(
        &mut self,
        form: Form,
        arguments: &[ast::Expr],
        name_span: Span,
        span: Span,
    ) -> Elab<Value> {
        self.runs_here(form, name_span)?;
        let Some(condition) = arguments.first() else {
            return self.fail(
                "L0244",
                format!("`{}!` takes a condition", form.name()),
                span,
            );
        };
        let written = self.message_argument(form, arguments, 1)?;
        let debug = form == Form::DebugAssert;
        let condition_value = self.check(condition, &Type::Bool)?;
        let (tested, negated) = self.tested(&condition_value, condition.span)?;
        let test = Test {
            test: tested.clone(),
            outcome: !negated,
        };
        let claim = self.asserted_claim(&test);
        // Rust's message names the condition as written; a written message
        // stands alone.
        let message = written
            .unwrap_or_else(|| format!("assertion failed: {}", self.spelled(condition.span)));
        let holds = if self.promises.no_panic {
            match self.solve(&claim, span, None) {
                Ok(proof) => Some(proof),
                Err(()) => {
                    let item = self.item_name.clone();
                    self.reword_unsolved(format!(
                        "`{}!` needs evidence of its condition in `{item}`, which promises no_panic",
                        form.name()
                    ));
                    return Err(());
                }
            }
        } else {
            self.try_solve(&claim, span)
        };
        let (then_fact, else_fact) = (HypId::fresh(), HypId::fresh());
        let fact = |holds: bool| Term::eq(Type::Bool, tested.clone(), Term::Bool(holds != negated));
        let unreachable = match holds {
            Some(proof) => {
                Some(self.refutation(&test, &claim, proof, else_fact, fact(false), span)?)
            }
            None => None,
        };
        let result = VarId::fresh();
        if !debug {
            // What the check teaches: the value of its statement, evidence
            // that the condition came out as it must have.
            self.declare_result(result, &Type::proof(fact(true)), span)?;
        }
        Ok(Value::new(
            Expr::Assert {
                debug,
                condition: Box::new(condition_value.expr),
                then_fact,
                else_fact,
                message,
                unreachable,
                result,
            },
            unit_type(),
        ))
    }

    /// The proposition a check decides, which is what its evidence is asked
    /// for: for a comparison of machine values, the claim reflection gives
    /// it, over the views, computed as the solver states a fact, or its
    /// negation; for an equality, the equality of the values themselves; and
    /// for any other `bool`, that it is `true`.
    fn asserted_claim(&mut self, test: &Test) -> Term {
        let Some((op, ty, a, b)) = test.parts() else {
            return Env::test_claim(test);
        };
        let (a, b) = (a.clone(), b.clone());
        let claim = match op {
            CmpOp::Eq => {
                let equal = Term::eq(Type::machine(ty), a, b);
                return if test.outcome {
                    equal
                } else {
                    self.prelude.not_prop(equal)
                };
            }
            _ => op.claim(Term::view(ty, a), Term::view(ty, b)),
        };
        let claim = if test.outcome {
            claim
        } else {
            self.prelude.not_prop(claim)
        };
        self.computed(&claim)
    }

    /// `False` in the arm where the check fails, from evidence of the
    /// condition and the arm's fact that the test came out the other way,
    /// checked in that arm's context.
    fn refutation(
        &mut self,
        test: &Test,
        claim: &Term,
        evidence: Proof,
        else_fact: HypId,
        failed: Term,
        span: Span,
    ) -> Elab<Proof> {
        let Some(holds) = self.test_evidence(claim, test, evidence) else {
            return self.internal(
                "the evidence of an assertion does not speak of its test",
                span,
            );
        };
        let fails = Proof::hyp(else_fact);
        let (holds, fails) = if test.outcome {
            (holds, fails)
        } else {
            (fails, holds)
        };
        let proof = contradiction(holds, fails);
        let mark = self.mark();
        let checked = self.assume(else_fact, failed, span).and_then(|()| {
            let falsehood = self.prelude.falsehood_prop();
            let checked = check_proof(&mut self.ctx, &proof, &falsehood);
            self.kernel(checked, span)
        });
        self.close(mark);
        checked.map(|()| proof)
    }

    /// The form is written where nothing runs, a formula or the body of a
    /// function of the logic: refused, or the function is elaborated again
    /// as an ordinary one (`items`, LOC-193).
    fn runs_here(&mut self, form: Form, name_span: Span) -> Elab<()> {
        if let Some(place) = self.formula {
            return self.fail(
                "L0239",
                format!(
                    "`{}!` cannot appear in {place}: nothing there runs",
                    form.name()
                ),
                name_span,
            );
        }
        if self.total {
            self.not_a_term
                .get_or_insert((format!("`{}!`", form.name()), name_span));
            return Err(());
        }
        Ok(())
    }

    /// The message of a form, the string literal at `first` among its
    /// arguments, when one is written. Rust's format arguments are not in
    /// Locus yet.
    fn message_argument(
        &mut self,
        form: Form,
        arguments: &[ast::Expr],
        first: usize,
    ) -> Elab<Option<String>> {
        match arguments.get(first..).unwrap_or_default() {
            [] => Ok(None),
            [
                ast::Expr {
                    kind: ExprKind::String(text),
                    ..
                },
            ] => Ok(Some(text.clone())),
            [other] => self.fail(
                "L0244",
                format!("the message of `{}!` is a string literal", form.name()),
                other.span,
            ),
            [_, second, ..] => {
                self.diagnostics.push(
                    Diagnostic::error(
                        "L0290",
                        format!(
                            "format arguments are not in Locus yet; `{}!` takes a string literal as its message, written as it is",
                            form.name()
                        ),
                        second.span,
                    )
                    .note("they arrive with the polish after the core (Target language)"),
                );
                Err(())
            }
        }
    }

    /// `L0239`: `panic!` or `todo!` in a function that promises `no_panic`,
    /// where every panic must be shown unreachable.
    #[inline(never)]
    fn panics_under_no_panic<T>(&mut self, form: Form, name_span: Span) -> Elab<T> {
        let item = &self.item_name;
        let (message, note) = match form {
            Form::Todo => (
                format!("`todo!` panics, and `{item}` promises no_panic"),
                "finish the body, or leave the promise off until it is finished",
            ),
            _ => (
                format!("`panic!` cannot appear in `{item}`, which promises no_panic"),
                "a function that promises no_panic has no panic it cannot show unreachable: `unreachable!()` takes that evidence, and `assert!(c)` evidence of `c`",
            ),
        };
        self.diagnostics
            .push(Diagnostic::error("L0239", message, name_span).note(note));
        Err(())
    }

    /// A hole tried where nothing demands it: the evidence when it is
    /// found, counted as a hole, and no trace otherwise.
    fn try_solve(&mut self, goal: &Term, span: Span) -> Option<Proof> {
        let (diagnostics, holes) = (self.diagnostics.len(), self.holes.len());
        match self.solve(goal, span, None) {
            Ok(proof) => Some(proof),
            Err(()) => {
                self.diagnostics.truncate(diagnostics);
                self.holes.truncate(holes);
                None
            }
        }
    }

    /// The unsolved-hole diagnostic `solve` just reported, as the form's
    /// own: `L0239`, with what the form needed before what could not be
    /// shown, and the hole's notes kept.
    fn reword_unsolved(&mut self, needed: String) {
        if let Some(last) = self.diagnostics.last_mut()
            && last.code == "L0230"
        {
            last.code = "L0239";
            last.message = format!("{needed}: {}", last.message);
        }
    }

    /// The source text of a span on one line, as a message quotes it.
    fn spelled(&self, span: Span) -> String {
        self.text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// `snapshot!(e)`: a `Ghost<T>` holding the logical value of `e: T`.
    /// It stands only where nothing runs, which is where a `Ghost<T>` value
    /// may be named, and `e` is elaborated as a logic-only context: every
    /// call in it is one a proposition admits, and every name is read.
    fn snapshot(
        &mut self,
        arguments: &[ast::Expr],
        expected: Option<&Type>,
        name_span: Span,
        span: Span,
    ) -> Elab<Value> {
        let [argument] = arguments else {
            return self.fail("L0208", "`snapshot!` takes one value", span);
        };
        if !self.reading() {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0201",
                    "`snapshot!` builds a `Ghost<T>`, which has no runtime form",
                    name_span,
                )
                .note("write it where nothing runs: `let g: Ghost<T> = snapshot!(x);` or `let g = snapshot!(x);`, in a `Ghost<T>` parameter or field, or in a proposition"),
            );
            return Err(());
        }
        let value = self.logical("the argument of `snapshot!`", |env| match expected {
            Some(expected) => env.check(argument, expected),
            None => env.infer(argument),
        })?;
        Ok(Value::new(Expr::Ghost(Box::new(value.expr)), value.ty))
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
        self.facts.push(Fact::new(proof, claim));
        Ok(())
    }

    /// The forms that have a spelling and no meaning yet, each with the
    /// task that gives it one.
    #[inline(never)]
    fn form_not_yet<T>(&mut self, form: Form, span: Span) -> Elab<T> {
        let arrives = match form {
            Form::Old => "references as parameters (O3, LOC-184)",
            Form::Recurse => "recursion (LOC-53)",
            Form::Matches => "the patterns it takes apart (LOC-71)",
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

/// `False` from `holds: test == true` and `fails: test == false`, for any
/// `test` of type `bool`: the two give `true == false`, which turns the
/// evaluated `0 == 0` at `u8`, `true`, into `false`, and reflection of that
/// test refutes `view(0) == view(0)`, which holds by reflexivity.
fn contradiction(holds: Proof, fails: Proof) -> Proof {
    let true_is_false = Proof::transport(
        holds,
        |hole| Term::eq(Type::Bool, hole, Term::Bool(false)),
        fails,
    );
    let zero = Term::U8(0);
    let same = Term::cmp(CmpOp::Eq, MachineInt::U8, zero.clone(), zero.clone());
    let same_is_false = Proof::transport(
        true_is_false,
        |hole| Term::eq(Type::Bool, same.clone(), hole),
        Proof::Evaluate(same.clone()),
    );
    let not_same = Proof::implies_elim(Proof::Axiom(Axiom::CmpReflect(same, false)), same_is_false);
    Proof::implies_elim(not_same, Proof::Refl(Term::view(MachineInt::U8, zero)))
}
