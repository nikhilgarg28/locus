//! The rule `linear`: a certificate of linear arithmetic over `Int`.
//!
//! A certificate is a goal, a positive coefficient for it, and a list of
//! pairs of a proof and a coefficient. Every constraint enters as the
//! conclusion of a proof the kernel checks in the ordinary way; nothing
//! enters on the word of whoever built the certificate. What is trusted
//! here, and stated in the kernel contract in `atlas.html`, is three steps:
//! reading a term of type `Int` as a linear form, reading a conclusion and
//! the goal as a constraint, and the sum. The rule does no search, divides
//! nothing, and rounds nothing.

use std::fmt;

use super::check::{expect_type, proof_claim, same};
use super::context::{Context, Mode};
use super::error::KernelError;
use super::int::Integer;
use super::term::{Prim, Proof, Term, Type};

/// The most pairs a certificate may have.
pub const MAX_LINEAR_PAIRS: usize = 256;

/// The most atoms a linear form may have, after atoms that cancel are
/// dropped.
pub const MAX_LINEAR_ATOMS: usize = 256;

/// The most bits a literal read by the rule may have: a coefficient, or an
/// `Int` literal inside a goal or a conclusion. The sums are not limited;
/// they are bounded by these counts.
pub const MAX_LINEAR_BITS: usize = 512;

/// Why a certificate was refused by the rule itself. A pair whose proof
/// fails is refused by the ordinary checker with that proof's error, not
/// with one of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinearError {
    /// The goal is neither `int_le(s, t)` nor the prelude's `False`.
    NotAGoal(Term),
    /// A conclusion is none of `int_le(s, t)`, `int_le(s, t) => False`, and
    /// `s ==[Int] t`.
    NotAConstraint(Term),
    /// The coefficient of the goal is not positive.
    GoalCoefficient(Integer),
    /// The coefficient of an inequality is negative.
    NegativeCoefficient(Integer),
    TooManyPairs(usize),
    TooManyAtoms(usize),
    LiteralTooLarge(Integer),
    /// The sum has an atom whose coefficient is not zero.
    Uncancelled(Term),
    /// Every atom cancels, but the constant of the sum is not negative.
    NotNegative(Integer),
}

impl fmt::Display for LinearError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAGoal(goal) => write!(f, "{goal} is not a goal the linear rule accepts"),
            Self::NotAConstraint(prop) => write!(f, "{prop} is not a linear constraint"),
            Self::GoalCoefficient(c) => write!(f, "the goal's coefficient {c} is not positive"),
            Self::NegativeCoefficient(c) => {
                write!(f, "the coefficient {c} of an inequality is negative")
            }
            Self::TooManyPairs(n) => write!(f, "{n} pairs exceed the limit of {MAX_LINEAR_PAIRS}"),
            Self::TooManyAtoms(n) => write!(f, "{n} atoms exceed the limit of {MAX_LINEAR_ATOMS}"),
            Self::LiteralTooLarge(n) => {
                write!(f, "the literal {n} exceeds {MAX_LINEAR_BITS} bits")
            }
            Self::Uncancelled(atom) => write!(f, "the atom {atom} does not cancel in the sum"),
            Self::NotNegative(c) => write!(f, "the sum is {c}, which is not negative"),
        }
    }
}

/// A constant plus a coefficient for each atom. No atom occurs twice, by
/// `same`, and no coefficient is zero.
struct Form {
    constant: Integer,
    atoms: Vec<(Term, Integer)>,
}

impl Form {
    fn constant(value: Integer) -> Self {
        Self {
            constant: value,
            atoms: Vec::new(),
        }
    }

    fn atom(term: Term) -> Self {
        Self {
            constant: Integer::zero(),
            atoms: vec![(term, Integer::from(1i64))],
        }
    }

    /// `self + scale * other`.
    fn add_scaled(&mut self, other: &Form, scale: &Integer) -> Result<(), LinearError> {
        self.constant = self.constant.add(&other.constant.mul(scale));
        for (atom, coefficient) in &other.atoms {
            let scaled = coefficient.mul(scale);
            match self.atoms.iter().position(|(mine, _)| same(mine, atom)) {
                Some(at) => {
                    let sum = self.atoms[at].1.add(&scaled);
                    if sum.is_zero() {
                        self.atoms.remove(at);
                    } else {
                        self.atoms[at].1 = sum;
                    }
                }
                None if scaled.is_zero() => {}
                None => self.atoms.push((atom.clone(), scaled)),
            }
        }
        if self.atoms.len() > MAX_LINEAR_ATOMS {
            return Err(LinearError::TooManyAtoms(self.atoms.len()));
        }
        Ok(())
    }
}

fn literal_ok(value: &Integer) -> Result<(), LinearError> {
    if value.magnitude().bit_length() > MAX_LINEAR_BITS {
        return Err(LinearError::LiteralTooLarge(value.clone()));
    }
    Ok(())
}

/// Reads a term of type `Int` as a linear form. Literals are constants;
/// `int_add`, `int_sub`, and `int_neg` are read through; `int_mul` is read
/// through when one side reads as a constant with no atoms. Every other
/// term is an atom.
fn read_form(term: &Term) -> Result<Form, LinearError> {
    let sides = |arguments: &[Term]| match arguments {
        [left, right] => Some((read_form(left), read_form(right))),
        _ => None,
    };
    match term {
        Term::Int(value) => {
            literal_ok(value)?;
            Ok(Form::constant(value.clone()))
        }
        Term::Prim(Prim::IntAdd, arguments) => match sides(arguments) {
            Some((left, right)) => {
                let mut form = left?;
                form.add_scaled(&right?, &Integer::from(1i64))?;
                Ok(form)
            }
            None => Ok(Form::atom(term.clone())),
        },
        Term::Prim(Prim::IntSub, arguments) => match sides(arguments) {
            Some((left, right)) => {
                let mut form = left?;
                form.add_scaled(&right?, &Integer::from(-1i64))?;
                Ok(form)
            }
            None => Ok(Form::atom(term.clone())),
        },
        Term::Prim(Prim::IntNeg, arguments) => match arguments.as_slice() {
            [inner] => {
                let mut form = Form::constant(Integer::zero());
                form.add_scaled(&read_form(inner)?, &Integer::from(-1i64))?;
                Ok(form)
            }
            _ => Ok(Form::atom(term.clone())),
        },
        Term::Prim(Prim::IntMul, arguments) => match sides(arguments) {
            Some((left, right)) => {
                let (left, right) = (left?, right?);
                let (scale, other) = if left.atoms.is_empty() {
                    (left.constant, right)
                } else if right.atoms.is_empty() {
                    (right.constant, left)
                } else {
                    return Ok(Form::atom(term.clone()));
                };
                let mut form = Form::constant(Integer::zero());
                form.add_scaled(&other, &scale)?;
                Ok(form)
            }
            None => Ok(Form::atom(term.clone())),
        },
        _ => Ok(Form::atom(term.clone())),
    }
}

/// `right - left`, the form that is non-negative when `left <= right` and
/// zero when `left == right`.
fn difference(left: &Term, right: &Term) -> Result<Form, LinearError> {
    let mut form = read_form(right)?;
    form.add_scaled(&read_form(left)?, &Integer::from(-1i64))?;
    Ok(form)
}

/// `left - right - 1`, the form that is non-negative when `left <= right`
/// is false: this is where discreteness enters.
fn negated(left: &Term, right: &Term) -> Result<Form, LinearError> {
    let mut form = difference(right, left)?;
    form.constant = form.constant.sub(&Integer::from(1i64));
    Ok(form)
}

/// `int_le(s, t)`, as its two sides.
fn comparison(prop: &Term) -> Option<(&Term, &Term)> {
    match prop {
        Term::Prim(Prim::IntLe, arguments) => match arguments.as_slice() {
            [left, right] => Some((left, right)),
            _ => None,
        },
        _ => None,
    }
}

/// Reads a conclusion as a constraint: the form that is non-negative, or
/// zero, when the conclusion holds. The flag says which.
fn read_constraint(ctx: &Context, prop: &Term) -> Result<(Form, bool), LinearError> {
    if let Some((left, right)) = comparison(prop) {
        return Ok((difference(left, right)?, false));
    }
    if let Term::Eq(Type::Int, left, right) = prop {
        return Ok((difference(left, right)?, true));
    }
    if let Term::Implies(premise, conclusion) = prop
        && let Some((left, right)) = comparison(premise)
        && let Some(prelude) = ctx.definitions().prelude()
        && same(conclusion, &prelude.falsehood_prop())
    {
        return Ok((negated(left, right)?, false));
    }
    Err(LinearError::NotAConstraint(prop.clone()))
}

/// The rule `linear`. Concludes `goal`.
pub(super) fn claim_of_linear(ctx: &mut Context, proof: &Proof) -> Result<Term, KernelError> {
    let Proof::Linear {
        goal,
        goal_coefficient,
        pairs,
    } = proof
    else {
        unreachable!("dispatched on this variant")
    };
    if pairs.len() > MAX_LINEAR_PAIRS {
        return Err(LinearError::TooManyPairs(pairs.len()).into());
    }
    expect_type(ctx, goal, &Type::Prop, Mode::Logical)?;
    // The negated goal, times its coefficient. `False` contributes nothing.
    let mut sum = Form::constant(Integer::zero());
    if let Some((left, right)) = comparison(goal) {
        literal_ok(goal_coefficient)?;
        if goal_coefficient.is_negative() || goal_coefficient.is_zero() {
            return Err(LinearError::GoalCoefficient(goal_coefficient.clone()).into());
        }
        sum.add_scaled(&negated(left, right)?, goal_coefficient)?;
    } else if !ctx
        .definitions()
        .prelude()
        .is_some_and(|prelude| same(goal, &prelude.falsehood_prop()))
    {
        return Err(LinearError::NotAGoal(goal.clone()).into());
    }
    for (proof, coefficient) in pairs {
        // What the proof proves is what is read: a stated conclusion is
        // never trusted.
        let conclusion = proof_claim(ctx, proof)?;
        let (form, is_equation) = read_constraint(ctx, &conclusion)?;
        literal_ok(coefficient)?;
        if !is_equation && coefficient.is_negative() {
            return Err(LinearError::NegativeCoefficient(coefficient.clone()).into());
        }
        sum.add_scaled(&form, coefficient)?;
    }
    // Each contribution is non-negative, or zero, when its constraint holds.
    // A sum of such things that is a negative constant refutes them together,
    // and the negated goal is the one that is not a checked proof.
    if let Some((atom, _)) = sum.atoms.first() {
        return Err(LinearError::Uncancelled(atom.clone()).into());
    }
    if !sum.constant.is_negative() {
        return Err(LinearError::NotNegative(sum.constant).into());
    }
    Ok(goal.clone())
}

/// The version of the text form below. It changes when the form does.
pub const LINEAR_TEXT_VERSION: u32 = 1;

/// The text form of a certificate, for storing one: the version, the goal
/// as the kernel prints terms, the goal's coefficient, and the pairs. The
/// kernel has no printer for proofs yet, so each pair's proof is written as
/// the name of its rule, with the axiom's name for an axiom; the full text
/// of a proof is E11's. Displays nothing for a proof that is not `Linear`.
pub struct CertificateText<'a>(pub &'a Proof);

impl fmt::Display for CertificateText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Proof::Linear {
            goal,
            goal_coefficient,
            pairs,
        } = self.0
        else {
            return Ok(());
        };
        write!(
            f,
            "linear v{LINEAR_TEXT_VERSION} {goal} ; {goal_coefficient} ; ["
        )?;
        for (index, (proof, coefficient)) in pairs.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            match proof {
                Proof::Axiom(axiom) => write!(f, "axiom {}", axiom.name())?,
                other => f.write_str(other.rule_name())?,
            }
            write!(f, " * {coefficient}")?;
        }
        f.write_str("]")
    }
}
