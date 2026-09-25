//! Nat is an ordinary checked logical product, not a new kernel axiom.
use super::{
    env::{Elab, Env, Fact, Global, StructInfo},
    exprs::Value,
};
use crate::{
    kernel::{Proof, Term, Type, VarId, check_proof},
    source::Span,
    typed::{Binder, Expr, Session, StructItem},
};
use std::rc::Rc;

pub(super) fn declare(session: &mut Session) -> Rc<StructInfo> {
    let value = Binder {
        id: VarId::fresh(),
        name: "value".into(),
        ty: Type::Int,
        ghost: true,
    };
    let evidence = Binder {
        id: VarId::fresh(),
        name: "nonnegative".into(),
        ty: Type::proof(Term::int_le(Term::int(0), Term::var(value.id))),
        ghost: true,
    };
    let fields = vec![value, evidence];
    let item = StructItem {
        name: "Nat".into(),
        fields: fields.clone(),
        derives: vec![],
    };
    let id = session
        .declare_struct(&item)
        .expect("Nat's dependent product is well formed");
    session
        .mark_logical_type(&Type::Struct(id))
        .expect("Nat has logical fields only");
    Rc::new(StructInfo {
        captures: Vec::new(),
        origin: None,
        id,
        name: "Nat".into(),
        fields,
        derives: vec![],
        visibility: None,
        field_visibility: vec![None, None],
    })
}
impl Env<'_> {
    pub(super) fn natural_type(&self) -> Type {
        let Some(Global::Struct(info)) = self.types.get("Nat") else {
            unreachable!("builtin Nat")
        };
        Type::Struct(info.id)
    }
    pub(super) fn is_natural(&self, ty: &Type) -> bool {
        *ty == self.natural_type()
    }
    pub(super) fn natural_integer(&mut self, value: Value, span: Span) -> Elab<Value> {
        if !self.is_natural(&value.ty) {
            return Ok(value);
        }
        let term = self.term(&value, span)?;
        let proof = Proof::OfTerm(Term::proj(term.clone(), 1));
        let inferred = crate::kernel::infer_proof(&mut self.ctx, &proof);
        let claim = self.kernel(inferred, span)?;
        self.facts.push(Fact::new(proof, claim));
        if let Expr::Struct { fields, .. } = value.expr {
            return Ok(Value::new(fields.into_iter().next().unwrap().1, Type::Int));
        }
        self.field(value, 0, Some("value".into()), span)
    }
    pub(super) fn make_natural(
        &mut self,
        value: Value,
        evidence: Option<Proof>,
        span: Span,
    ) -> Elab<Value> {
        let term = self.term(&value, span)?;
        let claim = Term::int_le(Term::int(0), term);
        let proof = match evidence {
            Some(proof) => proof,
            None => match self.stored_or(&claim, |env| env.discharge(&claim)) {
                Some((proof, _)) => proof,
                None => self.solve(&claim, span, None)?,
            },
        };
        let checked = check_proof(&mut self.ctx, &proof, &claim);
        self.kernel(checked, span)?;
        let Type::Struct(id) = self.natural_type() else {
            unreachable!()
        };
        Ok(Value::new(
            Expr::Struct {
                indices: Vec::new(),
                id,
                name: "Nat".into(),
                fields: vec![
                    ("value".into(), value.expr),
                    ("nonnegative".into(), Expr::Proof(proof)),
                ],
            },
            Type::Struct(id),
        ))
    }
}

/// Derive nonnegativity of truncated division using existing integer laws.
/// This proof is checked at every use; it introduces no arithmetic axiom.
pub(super) fn quotient_nonnegative(
    a: Term,
    b: Term,
    a_nonnegative: Proof,
    b_nonnegative: Proof,
    multiply_ordered: crate::kernel::FnId,
) -> Proof {
    use crate::kernel::{Axiom, derive::symm_at};
    let zero = Term::int(0);
    let q = Term::int_div(a.clone(), b.clone());
    let goal = Term::int_le(zero.clone(), q.clone());
    let positive_case_goal = goal.clone();
    Proof::CaseProof {
        scrutinee: Box::new(Proof::Axiom(Axiom::IntLeTotal(b.clone(), zero.clone()))),
        goal: goal.clone(),
        arms: vec![
            Proof::arm(1, 0, |payload, _| {
                let b_zero = Proof::implies_elim(
                    Proof::implies_elim(
                        Proof::Axiom(Axiom::IntLeAntisymm(b.clone(), zero.clone())),
                        Proof::OfTerm(payload[0].clone()),
                    ),
                    b_nonnegative.clone(),
                );
                let q_zero = Proof::Transport {
                    eq: Box::new(symm_at(&Type::Int, &b, b_zero)),
                    template: Term::eq(
                        Type::Int,
                        Term::int_div(a.clone(), Term::Bound(0)),
                        zero.clone(),
                    ),
                    proof: Box::new(Proof::Axiom(Axiom::IntDivZero(a.clone()))),
                };
                Proof::Transport {
                    eq: Box::new(symm_at(&Type::Int, &q, q_zero)),
                    template: Term::int_le(zero.clone(), Term::Bound(0)),
                    proof: Box::new(Proof::Axiom(Axiom::IntLeRefl(zero.clone()))),
                }
            }),
            Proof::arm(1, 0, |positive, _| {
                let upper = Proof::implies_elim(
                    Proof::Axiom(Axiom::IntRemUpperPos(a.clone(), b.clone())),
                    Proof::OfTerm(positive[0].clone()),
                );
                Proof::CaseProof {
                    scrutinee: Box::new(Proof::Axiom(Axiom::IntLeTotal(zero.clone(), q.clone()))),
                    goal: positive_case_goal.clone(),
                    arms: vec![
                        Proof::arm(1, 0, |payload, _| Proof::OfTerm(payload[0].clone())),
                        Proof::arm(1, 0, |negative, _| {
                            let q_le_negative_one = Proof::linear(
                                Term::int_le(q.clone(), Term::int(-1)),
                                1,
                                vec![(Proof::OfTerm(negative[0].clone()), 1)],
                            );
                            let product_bound = Proof::OfTerm(Term::call(
                                Term::Fn(multiply_ordered),
                                vec![
                                    q.clone(),
                                    Term::int(-1),
                                    b.clone(),
                                    Term::proof(q_le_negative_one),
                                    Term::proof(b_nonnegative.clone()),
                                ],
                            ));
                            let impossible = Proof::linear(
                                Term::int_lt(zero.clone(), zero.clone()),
                                1,
                                vec![
                                    (a_nonnegative.clone(), 1),
                                    (product_bound, 1),
                                    (upper, 1),
                                    (Proof::Axiom(Axiom::IntDivRem(a.clone(), b.clone())), 1),
                                ],
                            );
                            let falsehood = Proof::implies_elim(
                                Proof::Axiom(Axiom::IntLtIrrefl(zero.clone())),
                                impossible,
                            );
                            Proof::CaseProof {
                                scrutinee: Box::new(falsehood),
                                goal: goal.clone(),
                                arms: vec![],
                            }
                        }),
                    ],
                }
            }),
        ],
    }
}
