//! The table of primitive operations on the machine integer types: for each
//! of `+`, `-`, `*`, `/`, `%`, unary minus, and the wrapping methods, at
//! each type, the arity, the condition under which Rust panics, whether a
//! build without overflow checks wraps instead of panicking, and what the
//! result means. The condition and the meaning are stated over the model of
//! `src/kernel/machine.rs`: `view` takes each operand into `Int`, the exact
//! result is formed there with the primitives of `Int`, and `wrap` brings
//! it back into the type. Part of the trusted base: native evaluation
//! computes every row with `Row::compute`, and the axioms `op_model` and
//! `op_exact` state each row with `Row::model_statement` and
//! `Row::exact_statement`.
//!
//! The kernel evaluates the meaning that holds in every build, the wrapped
//! one, and never panics: a kernel term is a mathematical object. Panicking
//! is the interpreters' business; they read `Row::fits_at` regardless of build mode, while
//! `Row::rust_can_wrap` documents plain Rust. The check IR reads `Row::fits`
//! to state the obligation a `no_panic` promise makes.
//!
//! Each row is meant to be checked against the Rust Reference in a minute:
//! `+`, `-`, `*`, and unary minus panic on overflow in a build with overflow
//! checks and wrap in one without; `/` and `%` panic on a zero divisor, and
//! at a signed type on `min / -1` and `min % -1`, in every build; the
//! wrapping methods never panic. Unary minus exists at the signed types
//! only.

use super::defs::Prelude;
use super::int::Integer;
use super::machine::MachineInt;
use super::term::{Term, Type};

/// An operation of the table. The name is the Rust method or operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    /// `a + b`
    Add,
    /// `a - b`
    Sub,
    /// `a * b`
    Mul,
    /// `a / b`, truncated toward zero
    Div,
    /// `a % b`, the remainder of that division, with the sign of `a`
    Rem,
    /// `-a`, at the signed types only
    Neg,
    /// `a.wrapping_add(b)`
    WrappingAdd,
    /// `a.wrapping_sub(b)`
    WrappingSub,
    /// `a.wrapping_mul(b)`
    WrappingMul,
    /// `a.wrapping_neg()`, at the signed types only
    WrappingNeg,
}

/// When a row panics, as the Rust Reference states it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Panic {
    /// Never: the wrapping methods.
    Never,
    /// When the exact result lies outside `[min(T), max(T)]`: `+`, `-`,
    /// `*`, and unary minus. A build without overflow checks wraps instead.
    Overflow,
    /// When the divisor is zero, and, at a signed type, when the dividend is
    /// `min(T)` and the divisor is `-1`: `/` and `%`, in every build.
    Division,
}

impl Op {
    /// Every operation, in the order of the table.
    pub const ALL: [Op; 10] = [
        Self::Add,
        Self::Sub,
        Self::Mul,
        Self::Div,
        Self::Rem,
        Self::Neg,
        Self::WrappingAdd,
        Self::WrappingSub,
        Self::WrappingMul,
        Self::WrappingNeg,
    ];

    /// The name the kernel contract uses, which is Rust's method name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Rem => "rem",
            Self::Neg => "neg",
            Self::WrappingAdd => "wrapping_add",
            Self::WrappingSub => "wrapping_sub",
            Self::WrappingMul => "wrapping_mul",
            Self::WrappingNeg => "wrapping_neg",
        }
    }

    /// How the operation is written in Rust and in Locus.
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Neg => "-",
            Self::WrappingAdd => ".wrapping_add",
            Self::WrappingSub => ".wrapping_sub",
            Self::WrappingMul => ".wrapping_mul",
            Self::WrappingNeg => ".wrapping_neg",
        }
    }

    /// The number of operands: one for the negations, two otherwise.
    pub fn arity(self) -> usize {
        match self {
            Self::Neg | Self::WrappingNeg => 1,
            _ => 2,
        }
    }

    /// When the operation panics.
    pub fn panic(self) -> Panic {
        match self {
            Self::Add | Self::Sub | Self::Mul | Self::Neg => Panic::Overflow,
            Self::Div | Self::Rem => Panic::Division,
            Self::WrappingAdd | Self::WrappingSub | Self::WrappingMul | Self::WrappingNeg => {
                Panic::Never
            }
        }
    }

    /// Whether the table has a row for the operation at the type: every
    /// operation at every type, except the negations at an unsigned type.
    pub fn exists_at(self, ty: MachineInt) -> bool {
        match self {
            Self::Neg | Self::WrappingNeg => ty.signed(),
            _ => true,
        }
    }

    /// The row of the table, when there is one.
    pub fn row(self, ty: MachineInt) -> Option<Row> {
        self.exists_at(ty).then_some(Row { op: self, ty })
    }

    /// The exact result on `Int`, computed with `kernel::Integer`. The
    /// wrapping methods have the exact result of their plain counterparts;
    /// division and remainder are `Integer::div` and `Integer::rem`, which
    /// truncate toward zero and are total, `a / 0` being `0` and `a % 0`
    /// being `a`. Requires exactly `arity` operands.
    pub fn exact(self, operands: &[Integer]) -> Integer {
        match (self, operands) {
            (Self::Add | Self::WrappingAdd, [a, b]) => a.add(b),
            (Self::Sub | Self::WrappingSub, [a, b]) => a.sub(b),
            (Self::Mul | Self::WrappingMul, [a, b]) => a.mul(b),
            (Self::Div, [a, b]) => a.div(b),
            (Self::Rem, [a, b]) => a.rem(b),
            (Self::Neg | Self::WrappingNeg, [a]) => a.neg(),
            _ => panic!("{} takes {} operands", self.name(), self.arity()),
        }
    }

    /// The exact result as a term of `Int`, from the operands' views: the
    /// same operations as `exact`, as `int_add`, `int_sub`, `int_mul`,
    /// `int_div`, `int_rem`, and `int_neg`. Requires exactly `arity` views.
    pub fn exact_term(self, views: Vec<Term>) -> Term {
        let mut views = views.into_iter();
        let mut next = || views.next().expect("one view per operand");
        let term = match self {
            Self::Add | Self::WrappingAdd => Term::int_add(next(), next()),
            Self::Sub | Self::WrappingSub => Term::int_sub(next(), next()),
            Self::Mul | Self::WrappingMul => Term::int_mul(next(), next()),
            Self::Div => Term::int_div(next(), next()),
            Self::Rem => Term::int_rem(next(), next()),
            Self::Neg | Self::WrappingNeg => Term::int_neg(next()),
        };
        assert!(
            views.next().is_none(),
            "{} takes {} operands",
            self.name(),
            self.arity()
        );
        term
    }
}

/// One row of the table: an operation at a type. `Op::row` builds one for
/// each pair that exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Row {
    pub op: Op,
    pub ty: MachineInt,
}

impl Row {
    /// Every row of the table: each operation at each type it exists at.
    pub fn all() -> Vec<Row> {
        MachineInt::ALL
            .iter()
            .flat_map(|ty| Op::ALL.iter().filter_map(|op| op.row(*ty)))
            .collect()
    }

    pub fn arity(self) -> usize {
        self.op.arity()
    }

    pub fn panic(self) -> Panic {
        self.op.panic()
    }

    /// Whether a build without overflow checks computes the wrapped result
    /// where a build with them panics: true for the overflow of `+`, `-`,
    /// `*`, and unary minus, and false for `/` and `%`, which panic in
    /// every build, and for the wrapping methods, which never panic.
    pub fn rust_can_wrap(self) -> bool {
        self.panic() == Panic::Overflow
    }

    /// The meaning in every build, computed: the exact result reduced into
    /// the type by `MachineInt::wrap`. Total: at a pair that panics in Rust
    /// it is still a value, `wrap` of the total exact result, which for
    /// `min / -1` and `min % -1` is `min` and `0`, and for a zero divisor is
    /// `0` and `a`. Requires operands of the row's type, exactly `arity`
    /// of them.
    pub fn compute(self, operands: &[Integer]) -> Integer {
        debug_assert!(operands.iter().all(|value| self.ty.contains(value)));
        self.ty.wrap(&self.op.exact(operands))
    }

    /// Whether Rust computes a value at these operands rather than
    /// panicking in a build with overflow checks: for `+`, `-`, `*`, and
    /// unary minus, whether the exact result lies in the type's range; for
    /// `/` and `%`, whether the divisor is nonzero and, at a signed type,
    /// the pair is not `min` over `-1`; always for the wrapping methods.
    pub fn fits_at(self, operands: &[Integer]) -> bool {
        debug_assert_eq!(operands.len(), self.arity());
        match self.panic() {
            Panic::Never => true,
            Panic::Overflow => self.ty.contains(&self.op.exact(operands)),
            Panic::Division => {
                let (a, b) = (&operands[0], &operands[1]);
                let min_over_minus_one =
                    self.ty.signed() && *a == self.ty.min() && *b == Integer::from(-1i64);
                !b.is_zero() && !min_over_minus_one
            }
        }
    }

    /// The same condition as `fits_at`, as premises over the operands'
    /// views: a list of propositions that together say the operation does
    /// not panic, each one a comparison or the negation of one, so that a
    /// linear certificate can take it as a fact. Empty for the wrapping
    /// methods. For `+`, `-`, `*`, and unary minus: `min(T) <= e` and
    /// `e <= max(T)`, where `e` is the exact result of the views. For `/`
    /// and `%`: `view(b) == 0 => False`, and at a signed type also
    /// `view(a) == min(T) => (view(b) == -1 => False)`.
    pub fn fits(self, prelude: &Prelude, operands: &[Term]) -> Vec<Term> {
        assert_eq!(operands.len(), self.arity(), "{}", self.op.name());
        let int_eq = |left: Term, right: Term| Term::eq(Type::Int, left, right);
        match self.panic() {
            Panic::Never => Vec::new(),
            Panic::Overflow => {
                let exact = self.exact_term(operands);
                vec![
                    Term::int_le(Term::Int(self.ty.min()), exact.clone()),
                    Term::int_le(exact, Term::Int(self.ty.max())),
                ]
            }
            Panic::Division => {
                let (a, b) = (self.view(&operands[0]), self.view(&operands[1]));
                let mut premises = vec![prelude.not_prop(int_eq(b.clone(), Term::int(0)))];
                if self.ty.signed() {
                    premises.push(Term::implies(
                        int_eq(a, Term::Int(self.ty.min())),
                        prelude.not_prop(int_eq(b, Term::int(-1))),
                    ));
                }
                premises
            }
        }
    }

    /// `view[T](x)`.
    fn view(self, operand: &Term) -> Term {
        Term::view(self.ty, operand.clone())
    }

    /// The exact result as a term of `Int`: the row's operation on the
    /// views of the operands.
    pub fn exact_term(self, operands: &[Term]) -> Term {
        self.op
            .exact_term(operands.iter().map(|operand| self.view(operand)).collect())
    }

    /// The meaning in every build, as a term of `T`: `wrap[T]` of the exact
    /// result. This is what `compute` computes and what `op_model` states.
    pub fn meaning(self, operands: &[Term]) -> Term {
        Term::wrap(self.ty, self.exact_term(operands))
    }

    /// The operation applied, as a term: `op[T](operands)`.
    pub fn applied(self, operands: &[Term]) -> Term {
        Term::op(self.op, self.ty, operands.to_vec())
    }

    /// What `op_model[op, T](operands)` states:
    /// `op[T](operands) ==[T] wrap[T](e)`, where `e` is the exact result of
    /// the views. True in every build, of every row and every pair of
    /// operands, because it is how the result is computed; at a pair where
    /// Rust panics it speaks of a value the program never continues with.
    pub fn model_statement(self, operands: &[Term]) -> Term {
        Term::eq(
            Type::machine(self.ty),
            self.applied(operands),
            self.meaning(operands),
        )
    }

    /// What `op_exact[op, T](operands)` states, for the rows that can
    /// overflow: `min(T) <= e => (e <= max(T) => view[T](op[T](operands))
    /// ==[Int] e)`, where `e` is the exact result of the views. `None` for
    /// the other rows: the wrapping methods and `/` and `%`, whose meaning
    /// in every build is already the exact one whenever they do not panic,
    /// so `op_model` is all there is to say.
    pub fn exact_statement(self, operands: &[Term]) -> Option<Term> {
        if self.panic() != Panic::Overflow {
            return None;
        }
        let exact = self.exact_term(operands);
        let conclusion = Term::eq(Type::Int, self.view(&self.applied(operands)), exact.clone());
        Some(Term::implies(
            Term::int_le(Term::Int(self.ty.min()), exact.clone()),
            Term::implies(Term::int_le(exact, Term::Int(self.ty.max())), conclusion),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Integer, MachineInt, Op, Panic, Row};

    #[test]
    fn the_rows_are_the_operations_at_the_types_they_exist_at() {
        let rows = Row::all();
        // Ten operations at four signed types, eight at four unsigned ones.
        assert_eq!(rows.len(), 4 * 10 + 4 * 8);
        for ty in MachineInt::ALL {
            for op in Op::ALL {
                let expected = ty.signed() || !matches!(op, Op::Neg | Op::WrappingNeg);
                assert_eq!(
                    op.row(ty).is_some(),
                    expected,
                    "{}[{}]",
                    op.name(),
                    ty.name()
                );
            }
        }
        for row in rows {
            assert_eq!(row.rust_can_wrap(), row.panic() == Panic::Overflow);
            assert_eq!(
                row.panic() == Panic::Never,
                row.op.name().starts_with("wrapping_")
            );
        }
    }

    #[test]
    fn the_meaning_at_a_panicking_pair_is_a_value() {
        let int = |value: i128| Integer::from(value);
        for ty in [MachineInt::I8, MachineInt::I64] {
            let min = ty.min();
            let div = Op::Div.row(ty).unwrap();
            let rem = Op::Rem.row(ty).unwrap();
            assert!(!div.fits_at(&[min.clone(), int(-1)]));
            assert!(!rem.fits_at(&[min.clone(), int(-1)]));
            assert_eq!(div.compute(&[min.clone(), int(-1)]), min);
            assert_eq!(rem.compute(&[min.clone(), int(-1)]), int(0));
            assert!(!div.fits_at(&[int(7), int(0)]));
            assert_eq!(div.compute(&[int(7), int(0)]), int(0));
            assert_eq!(rem.compute(&[int(7), int(0)]), int(7));
            let neg = Op::Neg.row(ty).unwrap();
            assert!(!neg.fits_at(std::slice::from_ref(&min)));
            assert_eq!(neg.compute(std::slice::from_ref(&min)), min);
        }
    }
}
