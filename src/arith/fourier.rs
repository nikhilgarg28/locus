//! Fourier-Motzkin elimination with the bookkeeping that turns a refutation
//! into a certificate, and the back-substitution that turns consistency
//! into a point.
//!
//! The input is a set of rows, each a linear form over the atoms that is
//! either non-negative (an inequality) or zero (an equation) when its
//! constraint holds, together with the negated goal as one more inequality.
//! Every row carries its *multipliers*: the non-negative combination of the
//! original constraints it was derived from, so that a row's form is always
//! exactly the sum of those constraints' forms times those multipliers. The
//! original rows start with the multiplier 1 on themselves.
//!
//! Elimination removes one atom at a time. An equation with a coefficient on
//! the atom is used to substitute the atom out of every other row and is
//! then dropped, which uses it with either sign, as the rule allows. With no
//! equation left, each pair of an inequality with a positive coefficient and
//! one with a negative coefficient is added, scaled by the other's
//! magnitude, which uses both with positive multipliers, as the rule
//! requires of inequalities. A row that comes out with no atoms and a
//! negative constant is a refutation, and its multipliers are the
//! certificate: the kernel will add the same constraints times the same
//! coefficients and find the same negative constant.
//!
//! Every combined row is divided by the greatest common divisor of all its
//! numbers, coefficients, constant, and multipliers together, so that the
//! multipliers stay integers and the numbers stay small. Nothing is rounded:
//! elimination decides rational infeasibility, which is what one certificate
//! expresses. The order of elimination is fixed by counts and atom indices,
//! so the same problem takes the same steps on every machine.
//!
//! When every atom is eliminated and no refutation appeared, the rows are
//! satisfiable over the rationals, and the rows kept at each stage give a
//! point: the last atom eliminated is bounded by the rows of its stage, a
//! value is chosen for it, an integer when the bounds allow one, and the
//! stage before gives bounds for the atom before, and so on back. An
//! integral point satisfies every original row, atom by atom, and is a
//! counterexample within the fragment; a fractional coordinate is where the
//! driver branches.

use std::collections::BTreeMap;

use crate::kernel::Integer;

use super::form::{Form, Kind, gcd};
use super::rational::Rational;
use super::{Budget, Spent};

/// A constraint in the working set: its form, whether it is an inequality
/// or an equation, and the combination of original constraints it came
/// from. An equation's multipliers rest on equations alone; an inequality's
/// multipliers on inequalities are all non-negative.
#[derive(Clone, Debug)]
pub(super) struct Row {
    pub(super) form: Form,
    pub(super) kind: Kind,
    /// Original constraint index to its multiplier; nothing is zero.
    pub(super) multipliers: BTreeMap<usize, Integer>,
}

impl Row {
    /// An original constraint: the multiplier 1 on itself.
    pub(super) fn original(index: usize, form: Form, kind: Kind) -> Self {
        Self {
            form,
            kind,
            multipliers: BTreeMap::from([(index, Integer::from(1i64))]),
        }
    }

    /// `scale * self + other_scale * other`, in form and multipliers alike,
    /// divided by the gcd of everything.
    fn combine(&self, scale: &Integer, other: &Row, other_scale: &Integer, kind: Kind) -> Row {
        let mut form = Form::constant(Integer::zero());
        form.add_scaled(&self.form, scale);
        form.add_scaled(&other.form, other_scale);
        let mut multipliers: BTreeMap<usize, Integer> = BTreeMap::new();
        let mut add = |index: usize, value: Integer| {
            let total = match multipliers.get(&index) {
                Some(known) => known.add(&value),
                None => value,
            };
            if total.is_zero() {
                multipliers.remove(&index);
            } else {
                multipliers.insert(index, total);
            }
        };
        for (index, multiplier) in &self.multipliers {
            add(*index, multiplier.mul(scale));
        }
        for (index, multiplier) in &other.multipliers {
            add(*index, multiplier.mul(other_scale));
        }
        let mut row = Row {
            form,
            kind,
            multipliers,
        };
        row.normalize();
        row
    }

    /// Divides everything by the common divisor of everything. The
    /// multipliers are included, so they stay whole; the constant is
    /// included, so no rounding happens.
    fn normalize(&mut self) {
        let mut divisor = self.form.constant.clone();
        for coefficient in self.form.coefficients.values() {
            divisor = gcd(&divisor, coefficient);
        }
        for multiplier in self.multipliers.values() {
            divisor = gcd(&divisor, multiplier);
        }
        if divisor.is_zero() || divisor == Integer::from(1i64) {
            return;
        }
        self.form.constant = self.form.constant.div(&divisor);
        for coefficient in self.form.coefficients.values_mut() {
            *coefficient = coefficient.div(&divisor);
        }
        for multiplier in self.multipliers.values_mut() {
            *multiplier = multiplier.div(&divisor);
        }
    }

    /// The most bits in any number of the row.
    fn bits(&self) -> usize {
        self.form
            .coefficients
            .values()
            .chain(self.multipliers.values())
            .chain(std::iter::once(&self.form.constant))
            .map(|value| value.magnitude().bit_length())
            .max()
            .unwrap_or(0)
    }

    /// Whether the row, with no atoms left, refutes its own origins: an
    /// inequality with a negative constant, or an equation with a non-zero
    /// one.
    fn refutes(&self) -> bool {
        self.form.is_constant()
            && match self.kind {
                Kind::Inequality => self.form.constant.is_negative(),
                Kind::Equation => !self.form.constant.is_zero(),
            }
    }

    /// The certificate of a refuting row: its multipliers, negated for an
    /// equation whose constant is positive, which every multiplier of an
    /// equation row may be.
    fn certificate(&self) -> BTreeMap<usize, Integer> {
        if self.kind == Kind::Equation && !self.form.constant.is_negative() {
            return self
                .multipliers
                .iter()
                .map(|(index, multiplier)| (*index, multiplier.neg()))
                .collect();
        }
        self.multipliers.clone()
    }
}

/// The rows just before one atom was eliminated.
#[derive(Debug)]
pub(super) struct Stage {
    pub(super) atom: usize,
    pub(super) rows: Vec<Row>,
}

/// How an elimination ended.
#[derive(Debug)]
pub(super) enum Outcome {
    /// The rows are contradictory, and these multipliers on the original
    /// constraints show it.
    Refuted(BTreeMap<usize, Integer>),
    /// Every atom was eliminated and no contradiction appeared: the rows
    /// are satisfiable over the rationals, so no certificate exists. The
    /// stages, in the order of elimination, give a point by `point`.
    Consistent(Vec<Stage>),
    /// A count ran out.
    Budget { name: &'static str, limit: usize },
}

/// Eliminates every atom from `rows`, or stops at a refutation or a budget.
pub(super) fn eliminate(mut rows: Vec<Row>, budget: &Budget, spent: &mut Spent) -> Outcome {
    let mut stages = Vec::new();
    if let Some(outcome) = settle(&mut rows, &mut stages) {
        return outcome;
    }
    // Equations first: each removes one atom exactly, at the cost of one
    // combination per row that mentions the atom.
    while let Some(at) = rows
        .iter()
        .position(|row| row.kind == Kind::Equation && !row.form.is_constant())
    {
        // The atom with the smallest coefficient in magnitude keeps the
        // scaling small; the first such atom keeps the choice fixed.
        let (atom, coefficient) = rows[at]
            .form
            .coefficients
            .iter()
            .min_by_key(|(index, coefficient)| (coefficient.magnitude().clone(), **index))
            .map(|(index, coefficient)| (*index, coefficient.clone()))
            .expect("the equation has an atom");
        if spent.eliminations >= budget.eliminations {
            return Outcome::Budget {
                name: "eliminations",
                limit: budget.eliminations,
            };
        }
        spent.eliminations += 1;
        stages.push(Stage {
            atom,
            rows: rows.clone(),
        });
        let equation = rows.remove(at);
        // r' = |a| r - sign(a) b e cancels the atom: the coefficient on it
        // is |a| b - sign(a) b a = 0. The row keeps a positive multiplier,
        // and the equation is used with whichever sign is needed.
        let magnitude = Integer::from(coefficient.magnitude().clone());
        let sign = if coefficient.is_negative() {
            Integer::from(-1i64)
        } else {
            Integer::from(1i64)
        };
        let mut next = Vec::with_capacity(rows.len());
        for row in &rows {
            let b = row.form.coefficient(atom);
            if b.is_zero() {
                next.push(row.clone());
                continue;
            }
            if spent.derived >= budget.derived {
                return Outcome::Budget {
                    name: "derived",
                    limit: budget.derived,
                };
            }
            spent.derived += 1;
            let combined = row.combine(&magnitude, &equation, &sign.mul(&b).neg(), row.kind);
            if combined.bits() > budget.bits {
                return Outcome::Budget {
                    name: "bits",
                    limit: budget.bits,
                };
            }
            next.push(combined);
        }
        rows = next;
        if let Some(outcome) = settle(&mut rows, &mut stages) {
            return outcome;
        }
    }
    // Then the inequalities, one atom at a time.
    loop {
        let Some(atom) = cheapest_atom(&rows) else {
            return Outcome::Consistent(stages);
        };
        if spent.eliminations >= budget.eliminations {
            return Outcome::Budget {
                name: "eliminations",
                limit: budget.eliminations,
            };
        }
        spent.eliminations += 1;
        stages.push(Stage {
            atom,
            rows: rows.clone(),
        });
        let (mut positive, mut negative, mut rest) = (Vec::new(), Vec::new(), Vec::new());
        for row in rows {
            let coefficient = row.form.coefficient(atom);
            if coefficient.is_zero() {
                rest.push(row);
            } else if coefficient.is_negative() {
                negative.push((row, Integer::from(coefficient.magnitude().clone())));
            } else {
                positive.push((row, coefficient));
            }
        }
        // |n| p + |p| n has no atom left where p has coefficient |p| > 0 and
        // n has coefficient -|n| < 0, and both multipliers are positive.
        let pairs = positive.len() * negative.len();
        if spent.derived + pairs > budget.derived {
            return Outcome::Budget {
                name: "derived",
                limit: budget.derived,
            };
        }
        spent.derived += pairs;
        for (p, p_coefficient) in &positive {
            for (n, n_magnitude) in &negative {
                let combined = p.combine(n_magnitude, n, p_coefficient, Kind::Inequality);
                if combined.bits() > budget.bits {
                    return Outcome::Budget {
                        name: "bits",
                        limit: budget.bits,
                    };
                }
                rest.push(combined);
            }
        }
        rows = rest;
        if let Some(outcome) = settle(&mut rows, &mut stages) {
            return outcome;
        }
    }
}

/// Removes the rows with no atoms, ending at the first that refutes, and
/// the later duplicates of a form, which add nothing. Returns the outcome
/// when the elimination is over.
fn settle(rows: &mut Vec<Row>, stages: &mut Vec<Stage>) -> Option<Outcome> {
    let mut kept: Vec<Row> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        if row.refutes() {
            return Some(Outcome::Refuted(row.certificate()));
        }
        if row.form.is_constant() {
            continue;
        }
        if kept
            .iter()
            .any(|known| known.kind == row.kind && known.form == row.form)
        {
            continue;
        }
        kept.push(row);
    }
    *rows = kept;
    if rows.is_empty() {
        return Some(Outcome::Consistent(std::mem::take(stages)));
    }
    None
}

/// The atom whose elimination makes the fewest new rows, the lowest index
/// among equals. An atom with no positive or no negative coefficient costs
/// nothing: the rows that mention it are simply dropped, since that atom
/// can be chosen to satisfy them.
fn cheapest_atom(rows: &[Row]) -> Option<usize> {
    let mut counts: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for row in rows {
        for (atom, coefficient) in &row.form.coefficients {
            let entry = counts.entry(*atom).or_insert((0, 0));
            if coefficient.is_negative() {
                entry.1 += 1;
            } else {
                entry.0 += 1;
            }
        }
    }
    counts
        .iter()
        .min_by_key(|(atom, (positive, negative))| (positive * negative, **atom))
        .map(|(atom, _)| *atom)
}

/// A point of the consistent rows, by back-substitution through the
/// stages, last eliminated atom first. Each atom gets an integer when its
/// bounds at its stage allow one, the least such; an atom no row bounds
/// gets 0. The first atom whose bounds hold no integer ends the search:
/// the result is that atom and the floor of its bounds, where the driver
/// branches. Otherwise the result is the integral point, one value per
/// atom.
pub(super) fn point(stages: &[Stage], atoms: usize) -> Result<Vec<Integer>, (usize, Integer)> {
    let mut values: Vec<Option<Integer>> = vec![None; atoms];
    for stage in stages.iter().rev() {
        let atom = stage.atom;
        let (mut lower, mut upper): (Option<Rational>, Option<Rational>) = (None, None);
        for row in &stage.rows {
            let coefficient = row.form.coefficient(atom);
            if coefficient.is_zero() {
                continue;
            }
            // The row is `coefficient * atom + rest`, with the rest known:
            // it mentions only atoms eliminated later, which are assigned.
            let mut rest = row.form.constant.clone();
            for (other, c) in &row.form.coefficients {
                if *other != atom {
                    let value = values[*other].clone().unwrap_or_else(Integer::zero);
                    rest = rest.add(&c.mul(&value));
                }
            }
            // `coefficient * atom + rest >= 0`, or `== 0`: the bound is
            // `-rest / coefficient`, a lower bound when the coefficient is
            // positive and an upper bound when it is negative, and both for
            // an equation.
            let bound = Rational::new(rest.neg(), coefficient.clone());
            let (is_lower, is_upper) = match row.kind {
                Kind::Equation => (true, true),
                Kind::Inequality => (!coefficient.is_negative(), coefficient.is_negative()),
            };
            if is_lower && lower.as_ref().is_none_or(|known| bound > *known) {
                lower = Some(bound.clone());
            }
            if is_upper && upper.as_ref().is_none_or(|known| bound < *known) {
                upper = Some(bound);
            }
        }
        let value = match (&lower, &upper) {
            (Some(lower), Some(upper)) => {
                let least = lower.ceiling();
                if Rational::integer(least.clone()) <= *upper {
                    least
                } else {
                    return Err((atom, lower.floor()));
                }
            }
            (Some(lower), None) => lower.ceiling(),
            (None, Some(upper)) => upper.floor(),
            (None, None) => Integer::zero(),
        };
        values[atom] = Some(value);
    }
    Ok(values
        .into_iter()
        .map(|value| value.unwrap_or_else(Integer::zero))
        .collect())
}
