//! Linear forms over atoms, read from terms exactly as the kernel's rule
//! `linear` reads them (`src/kernel/linear.rs`). The procedure keeps its own
//! reading so that it can compute with the forms; nothing here is trusted,
//! and the certificate it leads to is checked by the kernel's reading alone.

use std::collections::BTreeMap;

use crate::kernel::{Integer, Prelude, Prim, Term, Type, same};

/// The atoms of a problem, in order of first occurrence. Two terms are one
/// atom exactly when the kernel calls them the same term, by `same`.
#[derive(Debug, Default)]
pub(super) struct Atoms {
    terms: Vec<Term>,
}

impl Atoms {
    /// The index of `term`, registering it when it is new.
    pub(super) fn intern(&mut self, term: &Term) -> usize {
        match self.terms.iter().position(|known| same(known, term)) {
            Some(index) => index,
            None => {
                self.terms.push(term.clone());
                self.terms.len() - 1
            }
        }
    }

    pub(super) fn term(&self, index: usize) -> &Term {
        &self.terms[index]
    }

    pub(super) fn len(&self) -> usize {
        self.terms.len()
    }
}

/// A constant plus a coefficient for each atom. No coefficient is zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Form {
    pub(super) constant: Integer,
    pub(super) coefficients: BTreeMap<usize, Integer>,
}

impl Form {
    pub(super) fn constant(value: Integer) -> Self {
        Self {
            constant: value,
            coefficients: BTreeMap::new(),
        }
    }

    pub(super) fn atom(index: usize) -> Self {
        Self {
            constant: Integer::zero(),
            coefficients: BTreeMap::from([(index, Integer::from(1i64))]),
        }
    }

    /// `self + scale * other`.
    pub(super) fn add_scaled(&mut self, other: &Form, scale: &Integer) {
        if scale.is_zero() {
            return;
        }
        self.constant = self.constant.add(&other.constant.mul(scale));
        for (atom, coefficient) in &other.coefficients {
            let scaled = coefficient.mul(scale);
            let total = match self.coefficients.get(atom) {
                Some(mine) => mine.add(&scaled),
                None => scaled,
            };
            if total.is_zero() {
                self.coefficients.remove(atom);
            } else {
                self.coefficients.insert(*atom, total);
            }
        }
    }

    pub(super) fn coefficient(&self, atom: usize) -> Integer {
        self.coefficients
            .get(&atom)
            .cloned()
            .unwrap_or_else(Integer::zero)
    }

    pub(super) fn is_constant(&self) -> bool {
        self.coefficients.is_empty()
    }

    /// The value of the form at an assignment to the atoms.
    pub(super) fn evaluate(&self, assignment: &[Integer]) -> Integer {
        let mut value = self.constant.clone();
        for (atom, coefficient) in &self.coefficients {
            value = value.add(&coefficient.mul(&assignment[*atom]));
        }
        value
    }
}

/// Reads a term of type `Int` as a linear form, as the kernel does:
/// literals are constants, `int_add`, `int_sub`, and `int_neg` are read
/// through, `int_mul` is read through when one side has no atoms, and
/// everything else is an atom.
pub(super) fn read_form(atoms: &mut Atoms, term: &Term) -> Form {
    let one = Integer::from(1i64);
    let minus_one = Integer::from(-1i64);
    match term {
        Term::Int(value) => Form::constant(value.clone()),
        Term::Prim(Prim::IntAdd, arguments) if arguments.len() == 2 => {
            let mut form = read_form(atoms, &arguments[0]);
            form.add_scaled(&read_form(atoms, &arguments[1]), &one);
            form
        }
        Term::Prim(Prim::IntSub, arguments) if arguments.len() == 2 => {
            let mut form = read_form(atoms, &arguments[0]);
            form.add_scaled(&read_form(atoms, &arguments[1]), &minus_one);
            form
        }
        Term::Prim(Prim::IntNeg, arguments) if arguments.len() == 1 => {
            let mut form = Form::constant(Integer::zero());
            form.add_scaled(&read_form(atoms, &arguments[0]), &minus_one);
            form
        }
        Term::Prim(Prim::IntMul, arguments) if arguments.len() == 2 => {
            // Read both sides before deciding, so that the atoms of a product
            // of two non-constant sides are still registered: the kernel
            // does not look inside such an atom, and neither does the
            // procedure, but their order of first occurrence is kept stable.
            let left = read_form(atoms, &arguments[0]);
            let right = read_form(atoms, &arguments[1]);
            let (scale, other) = if left.is_constant() {
                (left.constant, right)
            } else if right.is_constant() {
                (right.constant, left)
            } else {
                return Form::atom(atoms.intern(term));
            };
            let mut form = Form::constant(Integer::zero());
            form.add_scaled(&other, &scale);
            form
        }
        _ => Form::atom(atoms.intern(term)),
    }
}

/// Whether a constraint says its form is non-negative or zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// `form >= 0`; the certificate may use it with a non-negative
    /// coefficient only.
    Inequality,
    /// `form == 0`; the certificate may use it with either sign.
    Equation,
}

/// `int_le(s, t)`, as its two sides.
pub(super) fn comparison(prop: &Term) -> Option<(&Term, &Term)> {
    match prop {
        Term::Prim(Prim::IntLe, arguments) => match arguments.as_slice() {
            [left, right] => Some((left, right)),
            _ => None,
        },
        _ => None,
    }
}

/// `right - left`: non-negative when `left <= right`, zero when equal.
fn difference(atoms: &mut Atoms, left: &Term, right: &Term) -> Form {
    let mut form = read_form(atoms, right);
    form.add_scaled(&read_form(atoms, left), &Integer::from(-1i64));
    form
}

/// `left - right - 1`: non-negative exactly when `left <= right` fails.
/// This is where the discreteness of `Int` enters, as in the kernel.
pub(super) fn negation(atoms: &mut Atoms, left: &Term, right: &Term) -> Form {
    let mut form = difference(atoms, right, left);
    form.constant = form.constant.sub(&Integer::from(1i64));
    form
}

/// Reads a proposition as a constraint, in the three shapes the rule
/// accepts: `int_le(s, t)`, `int_le(s, t) => False`, and `s ==[Int] t`.
pub(super) fn read_constraint(
    atoms: &mut Atoms,
    prelude: Option<Prelude>,
    prop: &Term,
) -> Option<(Form, Kind)> {
    if let Some((left, right)) = comparison(prop) {
        return Some((difference(atoms, left, right), Kind::Inequality));
    }
    if let Term::Eq(Type::Int, left, right) = prop {
        return Some((difference(atoms, left, right), Kind::Equation));
    }
    if let Term::Implies(premise, conclusion) = prop
        && let Some((left, right)) = comparison(premise)
        && let Some(prelude) = prelude
        && same(conclusion, &prelude.falsehood_prop())
    {
        return Some((negation(atoms, left, right), Kind::Inequality));
    }
    None
}

/// The greatest common divisor of the magnitudes; `gcd(0, 0)` is `0`.
pub(super) fn gcd(left: &Integer, right: &Integer) -> Integer {
    let mut a = left.magnitude().clone();
    let mut b = right.magnitude().clone();
    while !b.is_zero() {
        let (_, remainder) = a.div_rem(&b).expect("the divisor is not zero");
        a = b;
        b = remainder;
    }
    Integer::from(a)
}
