//! Explicit proof guards for well-founded recursion over nonnegative Int.
use super::check::{check_telescope, expect_type, same};
use super::classical::term_is_classical;
use super::context::{Context, Mode};
use super::defs::{Definitions, FnDecl};
use super::depth::{Node, check_depth, push_children};
use super::error::KernelError;
use super::term::{CmpOp, FnId, Proof, Term, Type, VarId, field_type};
use std::rc::Rc;

impl Definitions {
    pub fn decrease_claim(&self, next: Term, current: Term) -> Result<Term, KernelError> {
        let prelude = self.prelude().ok_or(KernelError::NoPrelude)?;
        Ok(prelude.and_prop(
            Term::holds(Term::int_cmp(CmpOp::Le, Term::int(0), next.clone())),
            Term::holds(Term::int_cmp(CmpOp::Lt, next, current)),
        ))
    }

    pub fn declare_measured_fn(
        &mut self,
        signature: &Type,
        measure: usize,
        build: impl FnOnce(FnId, &[Term]) -> Term,
    ) -> Result<FnId, KernelError> {
        check_depth([signature.into()])?;
        let Type::Fn(params, result) = signature else {
            return Err(KernelError::NotAFunction(signature.clone()));
        };
        if params.get(measure) != Some(&Type::Int) {
            return Err(KernelError::InvalidRecursion(
                "the measure parameter must be Int",
            ));
        }
        let mut telescope = params.clone();
        telescope.push((**result).clone());
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        check_telescope(&mut ctx, &telescope)?;
        let id = FnId(self.fns.len());
        let vars: Vec<_> = params.iter().map(|_| VarId::fresh()).collect();
        let args: Vec<_> = vars.iter().copied().map(Term::Free).collect();
        let body = build(id, &args);
        check_depth([(&body).into()])?;
        let mut candidate = self.clone();
        candidate.fns.push(FnDecl {
            params: params.clone(),
            result: (**result).clone(),
            body: body.close_over(&vars),
            classical: false,
            executable: false,
        });
        let mut ctx = Context::with_definitions(Rc::new(candidate.clone()));
        for (index, var) in vars.iter().enumerate() {
            let ty = field_type(&telescope, index, |j| args[j].clone());
            ctx.declare_with(*var, ty, true)?;
        }
        let expected = field_type(&telescope, params.len(), |j| args[j].clone());
        expect_type(&mut ctx, &body, &expected, Mode::Logical)?;
        validate(&candidate, id, measure, &args[measure], (&body).into())?;
        candidate.fns[id.0].classical = term_is_classical(&candidate, &body);
        *self = candidate;
        Ok(id)
    }
}

fn self_call(term: &Term, recursive: FnId) -> Option<&[Term]> {
    match term {
        Term::Call(callee, args) if **callee == Term::Fn(recursive) => Some(args),
        Term::Proof(proof) => match &**proof {
            Proof::OfTerm(term) => self_call(term, recursive),
            _ => None,
        },
        _ => None,
    }
}

fn validate(
    defs: &Definitions,
    recursive: FnId,
    measure: usize,
    current: &Term,
    node: Node<'_>,
) -> Result<(), KernelError> {
    if let Node::Term(Term::Proj(tuple, 1)) = node
        && let Term::Tuple(types, values) = &**tuple
        && let ([Type::Proof(claim), result], [evidence, call]) =
            (types.as_slice(), values.as_slice())
        && let Some(arguments) = self_call(call, recursive)
    {
        let next = arguments.get(measure).ok_or(KernelError::InvalidRecursion(
            "recursive call lacks its measure",
        ))?;
        let expected = defs.decrease_claim(next.clone(), current.clone())?;
        if !same(claim, &expected) {
            return Err(KernelError::InvalidRecursion(
                "recursive guard must prove a nonnegative strictly smaller actual measure",
            ));
        }
        validate(defs, recursive, measure, current, evidence.into())?;
        validate(defs, recursive, measure, current, result.into())?;
        for argument in arguments {
            validate(defs, recursive, measure, current, argument.into())?;
        }
        return Ok(());
    }
    if matches!(node, Node::Term(Term::Fn(id)) if *id == recursive) {
        return Err(KernelError::InvalidRecursion(
            "a measured recursive call needs recurse!(descent_evidence, call)",
        ));
    }
    let mut children = Vec::new();
    push_children(node, &mut children);
    for child in children {
        validate(defs, recursive, measure, current, child)?;
    }
    Ok(())
}
