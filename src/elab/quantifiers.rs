//! Quantifier metadata recognizes complete checked schemas, never spellings.
use super::env::{Env, Global};
use crate::kernel::{Quantifiers, Term};
impl Env<'_> {
    pub(super) fn register_quantifiers(&mut self, names: &[super::generics::QuantifierNames]) {
        for names in names {
            let (Some(Global::Prop(exists)), Some(Global::Prop(forall))) =
                (self.types.get(&names.exists), self.types.get(&names.forall))
            else {
                continue;
            };
            let (exists, forall) = (exists.id, forall.id);
            if self
                .quantifiers
                .iter()
                .any(|pair| pair.exists_id() == exists)
            {
                continue;
            }
            let Ok(element) = self.ty(&names.element) else {
                continue;
            };
            match self.session.register_quantifiers(element, exists, forall) {
                Ok(pair) => self.quantifiers.push(pair),
                Err(error) => {
                    let _: super::env::Elab<()> = self.internal(error, names.element.span);
                }
            }
        }
    }
    pub(super) fn universal(&self, term: &Term) -> Option<(Quantifiers, Term)> {
        let Term::PropApp(id, args) = term else {
            return None;
        };
        let [predicate] = args.as_slice() else {
            return None;
        };
        self.quantifiers
            .iter()
            .find(|pair| pair.forall_id() == *id)
            .map(|pair| (pair.clone(), predicate.clone()))
    }
}

/// Lift a checked theorem function into library universal evidence. No native
/// quantifier proof node is created; every binder becomes a checked lambda.
pub(super) fn function_evidence(
    definitions: &crate::kernel::Definitions,
    quantifiers: &[Quantifiers],
    info: &super::env::FnInfo,
    function: crate::kernel::FnId,
) -> Option<crate::kernel::Proof> {
    use crate::kernel::{Proof, Type, VarId};
    fn close(
        definitions: &crate::kernel::Definitions,
        quantifiers: &[Quantifiers],
        info: &super::env::FnInfo,
        function: crate::kernel::FnId,
        given: Vec<Term>,
    ) -> Option<(Term, Proof)> {
        let substitute = |mut ty: Type| {
            for (binder, argument) in info.params.iter().zip(&given) {
                ty = ty.replace_var(binder.id, argument);
            }
            ty
        };
        let Some(parameter) = info.params.get(given.len()) else {
            let Type::Proof(claim) = substitute(info.result.clone()) else {
                return None;
            };
            return Some((*claim, Proof::OfTerm(Term::call(Term::Fn(function), given))));
        };
        let ty = substitute(parameter.ty.clone());
        if let Type::Proof(premise) = ty {
            let mut conclusion = None;
            let proof = Proof::implies_intro((*premise).clone(), |evidence| {
                let unavailable = evidence.clone();
                let mut given = given;
                given.push(Term::proof(evidence));
                match close(definitions, quantifiers, info, function, given) {
                    Some((claim, proof)) => {
                        conclusion = Some(claim);
                        proof
                    }
                    None => unavailable,
                }
            });
            return Some((Term::implies(*premise, conclusion?), proof));
        }
        let pair = quantifiers.iter().find(|pair| pair.element() == &ty)?;
        let variable = VarId::fresh();
        let mut given = given;
        given.push(Term::var(variable));
        let (claim, proof) = close(definitions, quantifiers, info, function, given)?;
        let parameters = [(variable, ty)];
        let predicate = Term::lambda_over(&parameters, &Type::Prop, claim.clone());
        let applied = Term::call(predicate.clone(), vec![Term::var(variable)]);
        let beta = Proof::transport(
            Proof::Definition(applied.clone()),
            |p| Term::eq(Type::Prop, p, applied.clone()),
            Proof::Refl(applied.clone()),
        );
        let proof = Proof::transport(beta, |proposition| proposition, proof);
        let proof_function =
            Term::lambda_over(&parameters, &Type::proof(applied), Term::proof(proof));
        let evidence = pair
            .each(definitions, predicate.clone(), proof_function)
            .ok()?;
        Some((pair.forall(predicate), evidence))
    }
    close(definitions, quantifiers, info, function, Vec::new()).map(|(_, proof)| proof)
}
