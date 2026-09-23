//! Derived quantifiers represented by ordinary checked library propositions.
//! These helpers add no rule: their results use Construct, CaseProof, Lambda,
//! and application and must pass the ordinary checker.
use super::check::same_types;
use super::{Definitions, KernelError, Proof, PropId, PropVariant, Term, Type};

#[derive(Clone, Debug)]
pub struct Quantifiers {
    element: Type,
    exists: PropId,
    forall: PropId,
}

impl Quantifiers {
    pub fn declare(defs: &mut Definitions, element: Type) -> Result<Self, KernelError> {
        if !defs.is_logical_type(&element) {
            return Err(KernelError::NotLogicalType(element));
        }
        let truth = defs.prelude().ok_or(KernelError::NoPrelude)?.truth_prop();
        let predicate = Type::Fn(vec![element.clone()], Box::new(Type::Prop));
        let exists_fields = Type::Tuple(vec![predicate.clone(), element.clone()]);
        let exists = defs.declare_prop(
            vec![predicate.clone()],
            vec![PropVariant::arm(exists_fields, |v| {
                Term::call(v[0].clone(), vec![v[1].clone()])
            })],
        )?;
        let forall_fields = Type::tuple(|v| match v {
            [] => Some(predicate.clone()),
            [p] => Some(Type::function(1, |v| {
                if v.is_empty() {
                    element.clone()
                } else {
                    Type::proof(Term::call(p.clone(), vec![v[0].clone()]))
                }
            })),
            _ => None,
        });
        let forall = defs.declare_prop(
            vec![predicate],
            vec![PropVariant::arm(forall_fields, |_| truth)],
        )?;
        Ok(Self {
            element,
            exists,
            forall,
        })
    }

    /// Recognize caller-supplied checked declarations by their full schemas.
    /// Names and declaration indices carry no semantic privilege.
    pub fn from_declarations(
        defs: &Definitions,
        element: Type,
        exists: PropId,
        forall: PropId,
    ) -> Result<Self, KernelError> {
        let mut expected = defs.clone();
        let schemas = Self::declare(&mut expected, element.clone())?;
        for (actual, schema) in [(exists, schemas.exists), (forall, schemas.forall)] {
            let actual = defs.props.get(actual.0).ok_or(KernelError::UnknownProp)?;
            let schema = &expected.props[schema.0];
            if actual.inductive
                || !same_types(&actual.params, &schema.params)
                || actual.variants.len() != 1
            {
                return Err(KernelError::NotUniversal(Term::PropApp(forall, vec![])));
            }
            let (a, s) = (&actual.variants[0], &schema.variants[0]);
            if !a.with_params || !a.conclusion.is_empty() || !same_types(&a.telescope, &s.telescope)
            {
                return Err(KernelError::NotUniversal(Term::PropApp(forall, vec![])));
            }
        }
        Ok(Self {
            element,
            exists,
            forall,
        })
    }

    pub fn element(&self) -> &Type {
        &self.element
    }
    pub fn exists_id(&self) -> PropId {
        self.exists
    }
    pub fn forall_id(&self) -> PropId {
        self.forall
    }
    pub fn exists(&self, predicate: Term) -> Term {
        Term::PropApp(self.exists, vec![predicate])
    }
    pub fn forall(&self, predicate: Term) -> Term {
        Term::PropApp(self.forall, vec![predicate])
    }

    pub fn witness(&self, predicate: Term, value: Term, evidence: Proof) -> Proof {
        Proof::Construct {
            prop: self.exists,
            variant: 0,
            params: vec![predicate],
            payload: vec![value, Term::proof(evidence)],
        }
    }

    pub fn each(
        &self,
        defs: &Definitions,
        predicate: Term,
        prove_each: Term,
    ) -> Result<Proof, KernelError> {
        let truth = defs.prelude().ok_or(KernelError::NoPrelude)?.truth;
        Ok(Proof::Construct {
            prop: self.forall,
            variant: 0,
            params: vec![predicate],
            payload: vec![
                prove_each,
                Term::proof(Proof::Construct {
                    prop: truth,
                    variant: 0,
                    params: vec![],
                    payload: vec![],
                }),
            ],
        })
    }

    pub fn specialize(&self, universal: Proof, predicate: Term, value: Term) -> Proof {
        let goal = Term::call(predicate, vec![value.clone()]);
        Proof::CaseProof {
            scrutinee: Box::new(universal),
            goal,
            arms: vec![Proof::arm(2, 0, |v, _| {
                Proof::OfTerm(Term::call(v[0].clone(), vec![value]))
            })],
        }
    }

    pub fn eliminate(
        &self,
        existential: Proof,
        goal: Term,
        body: impl FnOnce(Term, Proof) -> Proof,
    ) -> Proof {
        Proof::CaseProof {
            scrutinee: Box::new(existential),
            goal,
            arms: vec![Proof::arm(2, 0, |v, _| {
                body(v[0].clone(), Proof::OfTerm(v[1].clone()))
            })],
        }
    }
}

impl Definitions {
    /// Record only complete schemas validated against checked declarations.
    pub fn register_quantifiers(
        &mut self,
        element: Type,
        exists: PropId,
        forall: PropId,
    ) -> Result<Quantifiers, KernelError> {
        let pair = Quantifiers::from_declarations(self, element, exists, forall)?;
        if !self
            .quantifiers
            .iter()
            .any(|old| old.exists == exists && old.forall == forall)
        {
            self.quantifiers.push(pair.clone());
        }
        Ok(pair)
    }

    pub(super) fn quantifier_element(&self, id: PropId) -> Option<&Type> {
        self.quantifiers
            .iter()
            .find(|pair| pair.exists == id || pair.forall == id)
            .map(|pair| &pair.element)
    }
}
