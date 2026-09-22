//! Kernel types, terms, and proofs.
//!
//! Binding is locally nameless. A variable bound inside a term (by `Forall`,
//! by a transport template, by a `ForallIntro` proof, or by an earlier field
//! of a product type) is a de Bruijn index; a variable of the surrounding
//! context is a globally unique identity. Terms that differ only in the names
//! of bound variables are therefore equal as data, and substituting a
//! context-level term under a binder needs no shifting. Hypotheses bound by
//! `ImpliesIntro` are indexed separately from term variables.
//!
//! Types, terms, and proofs are mutually recursive: a proof type mentions a
//! proposition, a product value carries proofs in its proof fields, and a
//! proof may be a term of proof type.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use super::int::Integer;
use super::machine::MachineInt;
use super::nat::Natural;
use super::ops::Op;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn fresh_id() -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    assert!(id != u64::MAX, "kernel identity space exhausted");
    id
}

/// Identity of a context variable. Identities are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarId(u64);

impl VarId {
    pub fn fresh() -> Self {
        Self(fresh_id())
    }
}

/// Identity of a context hypothesis. Identities are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HypId(u64);

impl HypId {
    pub fn fresh() -> Self {
        Self(fresh_id())
    }
}

/// Identity of a declared struct; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructId(pub(super) usize);

/// Identity of a declared enum; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnumId(pub(super) usize);

/// Identity of a declared proposition; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PropId(pub(super) usize);

/// Identity of a declared math function; see `Definitions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FnId(pub(super) usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Bool,
    U8,
    /// Natural numbers. Internal to the kernel: the model of `u8` and the
    /// domain of induction. It has no runtime representation, so it is ghost.
    Nat,
    /// The integers of the logic. Like `Nat`, the type has no runtime
    /// representation, so it is ghost.
    Int,
    /// A machine integer type other than `u8`, which is `Type::U8`; the
    /// checker rejects `Machine(MachineInt::U8)`. Runtime data, like `u8`.
    Machine(MachineInt),
    /// The type of propositions. Every value of this type is ghost.
    Prop,
    /// `@P`: the type of proofs of the proposition `P`. Ghost.
    Proof(Box<Term>),
    /// A telescope of field types. Field `i` is under `i` binders:
    /// `Bound(0)` in it is field `i - 1`, `Bound(1)` is field `i - 2`, and so
    /// on. A term occurs in a type only inside a `Proof`, so a value can
    /// influence what a later proof field says and never what data is stored.
    Tuple(Vec<Type>),
    /// A declared struct. Nominal: two declarations are different types.
    Struct(StructId),
    /// A declared enum. Nominal.
    Enum(EnumId),
    /// A total function type. The parameters form a telescope and the result
    /// type is under all of them. Ordinary `fn` never reaches the kernel.
    Fn(Vec<Type>, Box<Type>),
}

impl Type {
    /// A ghost type has no runtime representation.
    pub fn is_ghost(&self) -> bool {
        match self {
            Self::Prop | Self::Proof(_) | Self::Nat | Self::Int => true,
            // A function into a ghost type is a proof or a predicate.
            Self::Fn(_, result) => result.is_ghost(),
            Self::Bool
            | Self::U8
            | Self::Machine(_)
            | Self::Tuple(_)
            | Self::Struct(_)
            | Self::Enum(_) => false,
        }
    }

    /// The kernel type of a machine integer type: `Type::U8` for `u8`, and
    /// `Type::Machine` for the other seven.
    pub fn machine(ty: MachineInt) -> Self {
        match ty {
            MachineInt::U8 => Self::U8,
            other => Self::Machine(other),
        }
    }

    /// Which machine integer type this is, if any.
    pub fn as_machine(&self) -> Option<MachineInt> {
        match self {
            Self::U8 => Some(MachineInt::U8),
            Self::Machine(ty) => Some(*ty),
            _ => None,
        }
    }

    pub fn proof(prop: Term) -> Self {
        Self::Proof(Box::new(prop))
    }

    /// Builds a telescope. `fields(earlier)` receives the earlier fields as
    /// terms and returns the type of the next field, or `None` when done.
    pub fn tuple(mut fields: impl FnMut(&[Term]) -> Option<Type>) -> Self {
        let mut vars: Vec<VarId> = Vec::new();
        let mut telescope = Vec::new();
        loop {
            let earlier: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
            let Some(ty) = fields(&earlier) else {
                return Self::Tuple(telescope);
            };
            telescope.push(ty.close_over(&vars));
            vars.push(VarId::fresh());
        }
    }

    /// Builds a function type of the given arity. `signature(params)` is
    /// called with 0, 1, ..., `arity` parameters in scope: the first `arity`
    /// calls return parameter types and the last returns the result type.
    pub fn function(arity: usize, mut signature: impl FnMut(&[Term]) -> Type) -> Self {
        let mut vars: Vec<VarId> = Vec::new();
        let mut telescope = Vec::new();
        loop {
            let earlier: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
            let ty = signature(&earlier).close_over(&vars);
            if vars.len() == arity {
                return Self::Fn(telescope, Box::new(ty));
            }
            telescope.push(ty);
            vars.push(VarId::fresh());
        }
    }

    /// The tuple telescope whose fields are the given variables, in order: a
    /// field's type may mention the variables before it.
    pub fn tuple_over(fields: &[(VarId, Type)]) -> Type {
        let mut vars = Vec::new();
        let mut telescope = Vec::new();
        for (var, ty) in fields {
            telescope.push(ty.close_over(&vars));
            vars.push(*var);
        }
        Self::Tuple(telescope)
    }

    /// The function type with the given parameters, in order, and a result
    /// type that may mention all of them.
    pub fn function_over(params: &[(VarId, Type)], result: &Type) -> Type {
        let Self::Tuple(telescope) = Self::tuple_over(params) else {
            unreachable!("tuple_over builds a tuple type")
        };
        let vars: Vec<VarId> = params.iter().map(|(var, _)| *var).collect();
        Self::Fn(telescope, Box::new(result.close_over(&vars)))
    }

    /// The type with every occurrence of the context variable replaced.
    pub fn replace_var(&self, var: VarId, replacement: &Term) -> Type {
        self.close_over(&[var]).open(replacement)
    }

    /// Replaces the outermost bound variable of a type.
    pub(super) fn open(&self, replacement: &Term) -> Type {
        self.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: 0,
                replacement,
            },
        )
    }

    /// Puts a type under binders for `vars`: variable `j` of `n` becomes
    /// index `n - 1 - j`.
    pub(super) fn close_over(&self, vars: &[VarId]) -> Type {
        let count = vars.len() as u32;
        let mut ty = self.clone();
        for (j, var) in vars.iter().enumerate() {
            ty = ty.rebind(Depth::at(count - 1 - j as u32), Rebind::CloseVar(*var));
        }
        ty
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Type {
        match self {
            Self::Bool
            | Self::U8
            | Self::Nat
            | Self::Int
            | Self::Machine(_)
            | Self::Prop
            | Self::Struct(_)
            | Self::Enum(_) => self.clone(),
            Self::Proof(prop) => Self::Proof(Box::new(prop.rebind(depth, op))),
            Self::Tuple(fields) => Self::Tuple(rebind_telescope(fields, depth, op)),
            Self::Fn(params, result) => Self::Fn(
                rebind_telescope(params, depth, op),
                Box::new(result.rebind(depth.under_vars(params.len() as u32), op)),
            ),
        }
    }
}

fn rebind_telescope(fields: &[Type], depth: Depth, op: Rebind<'_>) -> Vec<Type> {
    fields
        .iter()
        .enumerate()
        .map(|(index, field)| field.rebind(depth.under_vars(index as u32), op))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prim {
    /// `u8, u8 -> u8`
    WrappingAdd,
    WrappingSub,
    /// `u8, u8 -> bool`: the runtime comparisons.
    U8Eq,
    U8Lt,
    U8Le,
    /// `u8 -> Nat`: the model of a byte.
    ToNat,
    /// `Nat -> u8`: reduction modulo 256.
    OfNat,
    /// `Nat -> Nat`
    Succ,
    /// `Nat, Nat -> Nat`
    NatAdd,
    /// `Int, Int -> Int`
    IntAdd,
    IntSub,
    IntMul,
    /// The quotient truncated toward zero, as Rust's `/` on integers, and
    /// total: `a / 0` is `0`.
    IntDiv,
    /// The remainder of that division, as Rust's `%`: it has the sign of
    /// the dividend, and `a % 0` is `a`.
    IntRem,
    /// `Int -> Int`
    IntNeg,
    /// `Int, Int -> Prop`: the order of the integers. It is a proposition,
    /// not a runtime comparison, and the only primitive order on `Int`:
    /// `a < b` is written `a + 1 <= b`.
    IntLe,
    /// `T -> Int`: the mathematical value of a machine integer of type `T`.
    View(MachineInt),
    /// `Int -> T`: reduction into the range of `T`, modulo `2^bits`.
    Wrap(MachineInt),
    /// `S -> T`: what `as` between machine types compiles to; by axiom it
    /// is `wrap(T)` of `view(S)`.
    Cast(MachineInt, MachineInt),
    /// `T, T -> T`, or `T -> T` for the negations: a row of the table of
    /// primitive operations in `src/kernel/ops.rs`, `+`, `-`, `*`, `/`,
    /// `%`, unary minus, or a wrapping method at a machine type. Runtime
    /// data in, runtime data out. The negations exist at the signed types
    /// only; `Op::row` says which rows exist, and the checker rejects any
    /// other. Evaluation computes the meaning that holds in every build,
    /// `wrap[T]` of the exact result, and never panics.
    Op(Op, MachineInt),
}

impl Prim {
    /// The name the kernel contract uses. A primitive that is instantiated
    /// at a machine type is named without it; `Display` adds the type.
    pub fn name(self) -> &'static str {
        match self {
            Self::WrappingAdd => "wrapping_add",
            Self::WrappingSub => "wrapping_sub",
            Self::U8Eq => "u8_eq",
            Self::U8Lt => "u8_lt",
            Self::U8Le => "u8_le",
            Self::ToNat => "to_nat",
            Self::OfNat => "of_nat",
            Self::Succ => "succ",
            Self::NatAdd => "nat_add",
            Self::IntAdd => "int_add",
            Self::IntSub => "int_sub",
            Self::IntMul => "int_mul",
            Self::IntDiv => "int_div",
            Self::IntRem => "int_rem",
            Self::IntNeg => "int_neg",
            Self::IntLe => "int_le",
            Self::View(_) => "view",
            Self::Wrap(_) => "wrap",
            Self::Cast(..) => "cast",
            Self::Op(op, _) => op.name(),
        }
    }
}

impl fmt::Display for Prim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())?;
        match self {
            Self::View(ty) | Self::Wrap(ty) | Self::Op(_, ty) => write!(f, "[{}]", ty.name()),
            Self::Cast(from, to) => write!(f, "[{}, {}]", from.name(), to.name()),
            _ => Ok(()),
        }
    }
}

/// The axioms of the internal `Nat`, of the `u8` model, and of `Int`. Each
/// takes terms and yields a fixed proposition about them; see the kernel
/// contract in `atlas.html`, which names each axiom as `name` does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Axiom {
    /// `a + 0 == a`
    NatAddZero(Term),
    /// `a + succ(b) == succ(a + b)`
    NatAddSucc(Term, Term),
    /// `succ(a) == succ(b) => a == b`
    NatSuccInjective(Term, Term),
    /// `succ(a) == 0 => False`
    NatSuccNotZero(Term),
    /// `to_nat(x) < 256`
    ToNatBound(Term),
    /// `of_nat(to_nat(x)) == x`
    OfToNat(Term),
    /// `n < 256 => to_nat(of_nat(n)) == n`
    ToOfNat(Term),
    /// `of_nat(n + 256) == of_nat(n)`
    OfNatWrap(Term),
    /// `a.wrapping_add(b) == of_nat(to_nat(a) + to_nat(b))`
    WrappingAddModel(Term, Term),
    /// `a.wrapping_sub(b).wrapping_add(b) == a`
    WrappingSubModel(Term, Term),
    /// For a comparison `c` and its proposition `P`: `c == true => P` when
    /// the flag is true, `c == false => (P => False)` when it is false.
    Reflect(Term, bool),
    // The integers are a commutative ring. Below, `+`, `-`, `*`, and `<=`
    // stand for `int_add`, `int_sub` or `int_neg`, `int_mul`, and `int_le`.
    /// `(a + b) + c == a + (b + c)`
    IntAddAssoc(Term, Term, Term),
    /// `a + b == b + a`
    IntAddComm(Term, Term),
    /// `a + 0 == a`
    IntAddZero(Term),
    /// `a + (-a) == 0`
    IntAddNeg(Term),
    /// `a - b == a + (-b)`
    IntSubDef(Term, Term),
    /// `(a * b) * c == a * (b * c)`
    IntMulAssoc(Term, Term, Term),
    /// `a * b == b * a`
    IntMulComm(Term, Term),
    /// `a * 1 == a`
    IntMulOne(Term),
    /// `a * (b + c) == a * b + a * c`
    IntMulAdd(Term, Term, Term),
    /// `a <= a`
    IntLeRefl(Term),
    /// `a <= b => b <= c => a <= c`
    IntLeTrans(Term, Term, Term),
    /// `a <= b => b <= a => a == b`
    IntLeAntisymm(Term, Term),
    /// `a <= b => a + c <= b + c`
    IntLeAdd(Term, Term, Term),
    /// `0 <= a => 0 <= b => 0 <= a * b`
    IntLeMul(Term, Term),
    /// `a <= b || b + 1 <= a`: the order is total, and it is discrete,
    /// because the second case is `b < a` written out.
    IntLeTotal(Term, Term),
    /// `a + 1 <= a => False`
    IntLtIrrefl(Term),
    // Quotient and remainder, truncated toward zero. `/` and `%` stand for
    // `int_div` and `int_rem`, and `x < y` for `x + 1 <= y`, as
    // `Term::int_lt` builds it. Every one holds at every `a` and `b`, a zero
    // divisor included, under `a / 0 == 0` and `a % 0 == a`.
    /// `a == (a / b) * b + a % b`
    IntDivRem(Term, Term),
    /// `a / 0 == 0`
    IntDivZero(Term),
    /// `0 < b => -b < a % b`
    IntRemLowerPos(Term, Term),
    /// `0 < b => a % b < b`
    IntRemUpperPos(Term, Term),
    /// `b < 0 => b < a % b`
    IntRemLowerNeg(Term, Term),
    /// `b < 0 => a % b < -b`
    IntRemUpperNeg(Term, Term),
    /// `0 <= a => 0 <= a % b`
    IntRemNonneg(Term, Term),
    /// `a <= 0 => a % b <= 0`
    IntRemNonpos(Term, Term),
    // The model of each machine integer type `T` over `Int`: one schema,
    // instantiated at the type the axiom carries. `view`, `wrap`, and `cast`
    // stand for `view(T)`, `wrap(T)`, and `cast(S, T)`.
    /// `min(T) <= view(x)`, for `x : T`
    ViewLower(MachineInt, Term),
    /// `view(x) <= max(T)`, for `x : T`
    ViewUpper(MachineInt, Term),
    /// `wrap(view(x)) ==[T] x`, for `x : T`
    WrapView(MachineInt, Term),
    /// `min(T) <= n => (n <= max(T) => view(wrap(n)) == n)`, for `n : Int`
    ViewWrap(MachineInt, Term),
    /// `wrap(n + 2^bits) ==[T] wrap(n)`, for `n : Int`
    WrapPeriod(MachineInt, Term),
    /// `cast(S, T)(x) ==[T] wrap(T)(view(S)(x))`, for `x : S`
    CastDef(MachineInt, MachineInt, Term),
    // The table of primitive operations, `src/kernel/ops.rs`: two schemas,
    // instantiated at the row the axiom carries, an operation and a type,
    // and at the operands, of which there are as many as the row's arity,
    // each of type `T`. `e` stands for the exact result of the operands'
    // views on `Int`, as `Row::exact_term` builds it.
    /// `op[T](xs) ==[T] wrap[T](e)`, for every row: the meaning that holds
    /// in every build.
    OpModel(Op, MachineInt, Vec<Term>),
    /// `min(T) <= e => (e <= max(T) => view[T](op[T](xs)) ==[Int] e)`, for
    /// the rows that can overflow, `+`, `-`, `*`, and unary minus: the exact
    /// result, under the condition that it fits. Rejected at any other row.
    OpExact(Op, MachineInt, Vec<Term>),
}

impl Axiom {
    /// The name the kernel contract gives the axiom.
    pub fn name(&self) -> &'static str {
        match self {
            Self::NatAddZero(_) => "nat_add_zero",
            Self::NatAddSucc(..) => "nat_add_succ",
            Self::NatSuccInjective(..) => "nat_succ_injective",
            Self::NatSuccNotZero(_) => "nat_succ_not_zero",
            Self::ToNatBound(_) => "to_nat_bound",
            Self::OfToNat(_) => "of_to_nat",
            Self::ToOfNat(_) => "to_of_nat",
            Self::OfNatWrap(_) => "of_nat_wrap",
            Self::WrappingAddModel(..) => "wrapping_add_model",
            Self::WrappingSubModel(..) => "wrapping_sub_model",
            Self::Reflect(..) => "reflect",
            Self::IntAddAssoc(..) => "int_add_assoc",
            Self::IntAddComm(..) => "int_add_comm",
            Self::IntAddZero(_) => "int_add_zero",
            Self::IntAddNeg(_) => "int_add_neg",
            Self::IntSubDef(..) => "int_sub_def",
            Self::IntMulAssoc(..) => "int_mul_assoc",
            Self::IntMulComm(..) => "int_mul_comm",
            Self::IntMulOne(_) => "int_mul_one",
            Self::IntMulAdd(..) => "int_mul_add",
            Self::IntLeRefl(_) => "int_le_refl",
            Self::IntLeTrans(..) => "int_le_trans",
            Self::IntLeAntisymm(..) => "int_le_antisymm",
            Self::IntLeAdd(..) => "int_le_add",
            Self::IntLeMul(..) => "int_le_mul",
            Self::IntLeTotal(..) => "int_le_total",
            Self::IntLtIrrefl(_) => "int_lt_irrefl",
            Self::IntDivRem(..) => "int_div_rem",
            Self::IntDivZero(_) => "int_div_zero",
            Self::IntRemLowerPos(..) => "int_rem_lower_pos",
            Self::IntRemUpperPos(..) => "int_rem_upper_pos",
            Self::IntRemLowerNeg(..) => "int_rem_lower_neg",
            Self::IntRemUpperNeg(..) => "int_rem_upper_neg",
            Self::IntRemNonneg(..) => "int_rem_nonneg",
            Self::IntRemNonpos(..) => "int_rem_nonpos",
            Self::ViewLower(..) => "view_lower",
            Self::ViewUpper(..) => "view_upper",
            Self::WrapView(..) => "wrap_view",
            Self::ViewWrap(..) => "view_wrap",
            Self::WrapPeriod(..) => "wrap_period",
            Self::CastDef(..) => "cast_def",
            Self::OpModel(..) => "op_model",
            Self::OpExact(..) => "op_exact",
        }
    }

    fn map(&self, f: impl Fn(&Term) -> Term) -> Axiom {
        match self {
            Self::NatAddZero(a) => Self::NatAddZero(f(a)),
            Self::NatAddSucc(a, b) => Self::NatAddSucc(f(a), f(b)),
            Self::NatSuccInjective(a, b) => Self::NatSuccInjective(f(a), f(b)),
            Self::NatSuccNotZero(a) => Self::NatSuccNotZero(f(a)),
            Self::ToNatBound(x) => Self::ToNatBound(f(x)),
            Self::OfToNat(x) => Self::OfToNat(f(x)),
            Self::ToOfNat(n) => Self::ToOfNat(f(n)),
            Self::OfNatWrap(n) => Self::OfNatWrap(f(n)),
            Self::WrappingAddModel(a, b) => Self::WrappingAddModel(f(a), f(b)),
            Self::WrappingSubModel(a, b) => Self::WrappingSubModel(f(a), f(b)),
            Self::Reflect(c, flag) => Self::Reflect(f(c), *flag),
            Self::IntAddAssoc(a, b, c) => Self::IntAddAssoc(f(a), f(b), f(c)),
            Self::IntAddComm(a, b) => Self::IntAddComm(f(a), f(b)),
            Self::IntAddZero(a) => Self::IntAddZero(f(a)),
            Self::IntAddNeg(a) => Self::IntAddNeg(f(a)),
            Self::IntSubDef(a, b) => Self::IntSubDef(f(a), f(b)),
            Self::IntMulAssoc(a, b, c) => Self::IntMulAssoc(f(a), f(b), f(c)),
            Self::IntMulComm(a, b) => Self::IntMulComm(f(a), f(b)),
            Self::IntMulOne(a) => Self::IntMulOne(f(a)),
            Self::IntMulAdd(a, b, c) => Self::IntMulAdd(f(a), f(b), f(c)),
            Self::IntLeRefl(a) => Self::IntLeRefl(f(a)),
            Self::IntLeTrans(a, b, c) => Self::IntLeTrans(f(a), f(b), f(c)),
            Self::IntLeAntisymm(a, b) => Self::IntLeAntisymm(f(a), f(b)),
            Self::IntLeAdd(a, b, c) => Self::IntLeAdd(f(a), f(b), f(c)),
            Self::IntLeMul(a, b) => Self::IntLeMul(f(a), f(b)),
            Self::IntLeTotal(a, b) => Self::IntLeTotal(f(a), f(b)),
            Self::IntLtIrrefl(a) => Self::IntLtIrrefl(f(a)),
            Self::IntDivRem(a, b) => Self::IntDivRem(f(a), f(b)),
            Self::IntDivZero(a) => Self::IntDivZero(f(a)),
            Self::IntRemLowerPos(a, b) => Self::IntRemLowerPos(f(a), f(b)),
            Self::IntRemUpperPos(a, b) => Self::IntRemUpperPos(f(a), f(b)),
            Self::IntRemLowerNeg(a, b) => Self::IntRemLowerNeg(f(a), f(b)),
            Self::IntRemUpperNeg(a, b) => Self::IntRemUpperNeg(f(a), f(b)),
            Self::IntRemNonneg(a, b) => Self::IntRemNonneg(f(a), f(b)),
            Self::IntRemNonpos(a, b) => Self::IntRemNonpos(f(a), f(b)),
            Self::ViewLower(ty, x) => Self::ViewLower(*ty, f(x)),
            Self::ViewUpper(ty, x) => Self::ViewUpper(*ty, f(x)),
            Self::WrapView(ty, x) => Self::WrapView(*ty, f(x)),
            Self::ViewWrap(ty, n) => Self::ViewWrap(*ty, f(n)),
            Self::WrapPeriod(ty, n) => Self::WrapPeriod(*ty, f(n)),
            Self::CastDef(from, to, x) => Self::CastDef(*from, *to, f(x)),
            Self::OpModel(op, ty, xs) => Self::OpModel(*op, *ty, xs.iter().map(&f).collect()),
            Self::OpExact(op, ty, xs) => Self::OpExact(*op, *ty, xs.iter().map(&f).collect()),
        }
    }

    /// The terms the axiom is instantiated at, in order.
    pub fn terms(&self) -> Vec<&Term> {
        match self {
            Self::NatAddZero(a)
            | Self::NatSuccNotZero(a)
            | Self::ToNatBound(a)
            | Self::OfToNat(a)
            | Self::ToOfNat(a)
            | Self::OfNatWrap(a)
            | Self::Reflect(a, _)
            | Self::IntAddZero(a)
            | Self::IntAddNeg(a)
            | Self::IntMulOne(a)
            | Self::IntLeRefl(a)
            | Self::IntLtIrrefl(a)
            | Self::IntDivZero(a)
            | Self::ViewLower(_, a)
            | Self::ViewUpper(_, a)
            | Self::WrapView(_, a)
            | Self::ViewWrap(_, a)
            | Self::WrapPeriod(_, a)
            | Self::CastDef(_, _, a) => vec![a],
            Self::NatAddSucc(a, b)
            | Self::NatSuccInjective(a, b)
            | Self::WrappingAddModel(a, b)
            | Self::WrappingSubModel(a, b)
            | Self::IntAddComm(a, b)
            | Self::IntSubDef(a, b)
            | Self::IntMulComm(a, b)
            | Self::IntLeAntisymm(a, b)
            | Self::IntLeMul(a, b)
            | Self::IntLeTotal(a, b)
            | Self::IntDivRem(a, b)
            | Self::IntRemLowerPos(a, b)
            | Self::IntRemUpperPos(a, b)
            | Self::IntRemLowerNeg(a, b)
            | Self::IntRemUpperNeg(a, b)
            | Self::IntRemNonneg(a, b)
            | Self::IntRemNonpos(a, b) => vec![a, b],
            Self::IntAddAssoc(a, b, c)
            | Self::IntMulAssoc(a, b, c)
            | Self::IntMulAdd(a, b, c)
            | Self::IntLeTrans(a, b, c)
            | Self::IntLeAdd(a, b, c) => vec![a, b, c],
            Self::OpModel(_, _, xs) | Self::OpExact(_, _, xs) => xs.iter().collect(),
        }
    }
}

/// Data terms and propositions. A proposition is a term of type `Prop`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Term {
    Free(VarId),
    Bound(u32),
    Bool(bool),
    U8(u8),
    /// A `Nat` literal, of arbitrary size.
    Nat(Natural),
    /// An `Int` literal, of arbitrary size and either sign.
    Int(Integer),
    /// A literal of a machine integer type other than `u8`, whose literals
    /// are `U8`. The value must lie in the range of the type: `Term::machine`
    /// builds only such literals, and the checker rejects any other.
    Machine(MachineInt, Integer),
    Prim(Prim, Vec<Term>),
    /// `a == b` at the given type.
    Eq(Type, Box<Term>, Box<Term>),
    Implies(Box<Term>, Box<Term>),
    /// Binds `Bound(0)` in its body.
    Forall(Type, Box<Term>),
    /// A tuple value. It carries its telescope because a dependent product
    /// type cannot be inferred from the values alone.
    Tuple(Vec<Type>, Vec<Term>),
    Struct(StructId, Vec<Term>),
    /// Positional projection from a tuple or struct.
    Proj(Box<Term>, usize),
    /// A proof used as a value, of type `@P`. Every proof field of a product
    /// value has this form, which is what lets comparison ignore proofs
    /// without knowing any types.
    Proof(Box<Proof>),
    /// A declared math function used as a value.
    Fn(FnId),
    /// Application of a term of function type.
    Call(Box<Term>, Vec<Term>),
    /// A value of a declared enum: the variant's index and its payload.
    Variant(EnumId, usize, Vec<Term>),
    /// Case analysis on a `bool` (arms: false, true) or an enum (one arm per
    /// variant, in declaration order). The result type does not depend on
    /// the scrutinee and is not a proof type.
    Case {
        scrutinee: Box<Term>,
        result: Type,
        arms: Vec<TermArm>,
    },
    /// A declared proposition applied to its arguments.
    PropApp(PropId, Vec<Term>),
    /// Binds `Bound(0)` in its body.
    Exists(Type, Box<Term>),
    /// A value of any type, from a proof of a proposition with no variants.
    /// It marks a point that is never reached.
    Absurd(Box<Proof>, Type),
    /// Iteration over the byte range `lo..hi`, the term-level recursion rule.
    For(Box<ForLoop>),
}

/// `for i in lo..hi (state = init) { body }`.
///
/// The state is a tuple whose telescope is under one binder, the index, so
/// an invariant may relate the state to the progress made. `body` is under
/// two term binders, the index (`Bound(1)`) and the current state
/// (`Bound(0)`), and two hypothesis binders, `lo <= i` (`Bound(1)`) and
/// `i < hi` (`Bound(0)`). It produces the state for index `i + 1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForLoop {
    pub lo: Term,
    pub hi: Term,
    /// Proves `u8_le(lo, hi)`.
    pub ordered: Proof,
    pub state: Vec<Type>,
    pub init: Term,
    pub body: Term,
}

/// Builds the body of a term-level case arm from its payload variables and
/// the proof that the scrutinee is this arm's variant applied to them.
pub type ArmBuilder<'a> = Box<dyn FnOnce(&[Term], Proof) -> Term + 'a>;

/// An arm of a term-level case. The body is under `binders` term binders, one
/// per payload field of the variant, and one hypothesis binder: the fact that
/// the scrutinee equals this variant applied to the payload. A branch of a
/// math function therefore knows what a branch of executable code knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TermArm {
    pub binders: u32,
    pub body: Term,
}

/// An arm of a proof-level case. The body is under `vars` term binders and
/// `hyps` hypothesis binders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofArm {
    pub vars: u32,
    pub hyps: u32,
    pub body: Box<Proof>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypRef {
    Free(HypId),
    Bound(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    Hyp(HypRef),
    /// A term of type `@P`, such as a projection of a proof field, proves `P`.
    OfTerm(Term),
    Refl(Term),
    /// From `eq: a == b` and `proof: template[a]`, conclude `template[b]`.
    /// The template binds `Bound(0)` as its hole.
    Transport {
        eq: Box<Proof>,
        template: Term,
        proof: Box<Proof>,
    },
    /// Binds hypothesis `Bound(0)` in its body.
    ImpliesIntro {
        hyp: Term,
        body: Box<Proof>,
    },
    ImpliesElim(Box<Proof>, Box<Proof>),
    /// Binds term variable `Bound(0)` in its body.
    ForallIntro {
        ty: Type,
        body: Box<Proof>,
    },
    ForallElim(Box<Proof>, Term),
    /// Computation axiom: `(v_0, ..., v_n).i == v_i`, for the given
    /// projection term.
    Projection(Term),
    /// Computation axiom: `op(literals) == literal`, by native evaluation.
    Literal(Term),
    /// Computation axiom: `f(args) == body[params := args]`, the defining
    /// equation of a declared math function, for the given call.
    Definition(Term),
    /// Computation axiom: a case on a known constructor equals its arm.
    CaseStep(Term),
    /// A constructor of a declared proposition. `params` instantiates the
    /// proposition's parameters for a variant without a stated conclusion and
    /// is empty for a variant with one.
    Construct {
        prop: PropId,
        variant: usize,
        params: Vec<Term>,
        payload: Vec<Term>,
    },
    /// Case analysis on a proof of a declared proposition. Each arm binds its
    /// variant's payload and, for a variant with a stated conclusion, one
    /// index equation per parameter.
    CaseProof {
        scrutinee: Box<Proof>,
        goal: Term,
        arms: Vec<ProofArm>,
    },
    /// Case analysis on data, to prove a goal. Each arm binds its variant's
    /// payload and the hypothesis `scrutinee == variant(payload)`.
    CaseData {
        scrutinee: Term,
        goal: Term,
        arms: Vec<ProofArm>,
    },
    /// `prop` is `exists (x: A) { B }`; `proof` proves `B[witness]`.
    ExistsIntro {
        prop: Term,
        witness: Term,
        proof: Box<Proof>,
    },
    /// The arm binds the witness and the hypothesis that it satisfies the
    /// body. The goal cannot mention the witness.
    ExistsElim {
        exists: Box<Proof>,
        goal: Term,
        arm: ProofArm,
    },
    /// `p || !p`, for the prelude's `Or` and `False`.
    ExcludedMiddle(Term),
    /// Computation axiom: a `for` over an empty range equals its initial
    /// state.
    ForEmpty(Term),
    /// Computation axiom: a `for` whose upper bound is `h.wrapping_add(1)`
    /// equals its body at index `h` applied to the `for` up to `h`. `lower`
    /// proves `lo <= h` and `upper` proves `h < h.wrapping_add(1)`.
    ForStep {
        looped: Term,
        lower: Box<Proof>,
        upper: Box<Proof>,
    },
    /// A proof the evaluator discarded. It proves nothing: checking it is an
    /// error. It exists so that evaluation can drop the contents of proofs,
    /// which it never inspects, without changing the shape of a value.
    Omitted,
    /// Big-step evaluation of a closed term whose type is plain data.
    Evaluate(Term),
    /// `forall (x: u8) { body == true }`, by evaluating all 256 cases. The
    /// body binds `Bound(0)`.
    EvaluateAll(Term),
    /// An axiom of `Nat`, of the `u8` model, or of `Int`.
    Axiom(Axiom),
    /// Induction over `Nat`. The motive binds `Bound(0)`; `base` proves
    /// `motive[0]`; `step` binds `n` and the hypothesis `motive[n]` and
    /// proves `motive[succ(n)]`. Concludes `motive[target]`.
    NatInduction {
        motive: Term,
        base: Box<Proof>,
        step: ProofArm,
        target: Term,
    },
    /// Induction over the non-negative integers. The motive binds
    /// `Bound(0)`; `base` proves `motive[0]`; `step` binds `n` and the
    /// hypotheses `0 <= n` and `motive[n]`, in that order, and proves
    /// `motive[n + 1]`. Concludes `0 <= target => motive[target]`.
    IntInduction {
        motive: Term,
        base: Box<Proof>,
        step: ProofArm,
        target: Term,
    },
    /// A certificate of linear arithmetic over `Int`, checked by
    /// `linear.rs`: the negated goal times `goal_coefficient`, plus the
    /// conclusion of each pair's proof times its coefficient, add up to a
    /// negative constant. Concludes `goal`.
    Linear {
        goal: Term,
        goal_coefficient: Integer,
        pairs: Vec<(Proof, Integer)>,
    },
}

/// How many binders of each kind enclose the current position.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Depth {
    vars: u32,
    hyps: u32,
}

impl Depth {
    fn at(vars: u32) -> Self {
        Self { vars, hyps: 0 }
    }

    fn under_hyps(self, count: u32) -> Self {
        Self {
            hyps: self.hyps + count,
            ..self
        }
    }

    fn under_vars(self, count: u32) -> Self {
        Self {
            vars: self.vars + count,
            ..self
        }
    }

    pub(super) fn under(self, vars: u32, hyps: u32) -> Self {
        self.under_vars(vars).under_hyps(hyps)
    }

    fn under_hyp(self) -> Self {
        Self {
            hyps: self.hyps + 1,
            ..self
        }
    }
}

/// One traversal serves opening and closing of both kinds of binder.
#[derive(Clone, Copy)]
pub(super) enum Rebind<'a> {
    /// Replace the bound variable `index` binders out with a locally closed
    /// term. Other indices are left alone.
    OpenVar {
        index: u32,
        replacement: &'a Term,
    },
    CloseVar(VarId),
    /// Replace the bound hypothesis `index` binders out.
    OpenHyp {
        index: u32,
        id: HypId,
    },
    /// Replace the bound hypothesis `index` binders out with a locally
    /// closed proof.
    SubstHyp {
        index: u32,
        replacement: &'a Proof,
    },
    CloseHyp(HypId),
}

impl Term {
    pub fn var(id: VarId) -> Self {
        Self::Free(id)
    }

    pub fn eq(ty: Type, left: Term, right: Term) -> Self {
        Self::Eq(ty, Box::new(left), Box::new(right))
    }

    pub fn implies(premise: Term, conclusion: Term) -> Self {
        Self::Implies(Box::new(premise), Box::new(conclusion))
    }

    /// Builds `forall (x: ty) { body(x) }`.
    pub fn forall(ty: Type, body: impl FnOnce(Term) -> Term) -> Self {
        let var = VarId::fresh();
        let body = body(Self::Free(var)).close(var);
        Self::Forall(ty, Box::new(body))
    }

    pub fn wrapping_add(left: Term, right: Term) -> Self {
        Self::Prim(Prim::WrappingAdd, vec![left, right])
    }

    pub fn wrapping_sub(left: Term, right: Term) -> Self {
        Self::Prim(Prim::WrappingSub, vec![left, right])
    }

    pub fn prim(prim: Prim, arguments: Vec<Term>) -> Self {
        Self::Prim(prim, arguments)
    }

    /// A `Nat` literal.
    pub fn nat(value: u64) -> Self {
        Self::Nat(Natural::from(value))
    }

    pub fn to_nat(byte: Term) -> Self {
        Self::Prim(Prim::ToNat, vec![byte])
    }

    pub fn of_nat(number: Term) -> Self {
        Self::Prim(Prim::OfNat, vec![number])
    }

    pub fn succ(number: Term) -> Self {
        Self::Prim(Prim::Succ, vec![number])
    }

    pub fn nat_add(left: Term, right: Term) -> Self {
        Self::Prim(Prim::NatAdd, vec![left, right])
    }

    /// An `Int` literal.
    pub fn int(value: i64) -> Self {
        Self::Int(Integer::from(value))
    }

    pub fn int_add(left: Term, right: Term) -> Self {
        Self::Prim(Prim::IntAdd, vec![left, right])
    }

    pub fn int_sub(left: Term, right: Term) -> Self {
        Self::Prim(Prim::IntSub, vec![left, right])
    }

    pub fn int_mul(left: Term, right: Term) -> Self {
        Self::Prim(Prim::IntMul, vec![left, right])
    }

    /// `left / right` over `Int`, truncated toward zero.
    pub fn int_div(left: Term, right: Term) -> Self {
        Self::Prim(Prim::IntDiv, vec![left, right])
    }

    /// `left % right` over `Int`, the remainder of `int_div`.
    pub fn int_rem(left: Term, right: Term) -> Self {
        Self::Prim(Prim::IntRem, vec![left, right])
    }

    pub fn int_neg(number: Term) -> Self {
        Self::Prim(Prim::IntNeg, vec![number])
    }

    /// The proposition `left <= right` over `Int`.
    pub fn int_le(left: Term, right: Term) -> Self {
        Self::Prim(Prim::IntLe, vec![left, right])
    }

    /// `left < right` over `Int` is not a form of its own: it abbreviates
    /// `left + 1 <= right`, which is what makes the order discrete.
    pub fn int_lt(left: Term, right: Term) -> Self {
        Self::int_le(Self::int_add(left, Self::int(1)), right)
    }

    /// The literal of a machine integer type: `U8` for `u8`, `Machine` for
    /// the others. The value must be in the range of the type.
    pub fn machine(ty: MachineInt, value: Integer) -> Self {
        assert!(
            ty.contains(&value),
            "Term::machine: {value} is not a value of {}",
            ty.name()
        );
        match ty {
            MachineInt::U8 => {
                let byte = value.to_i128().and_then(|v| u8::try_from(v).ok());
                Self::U8(byte.expect("a value of u8 is a byte"))
            }
            other => Self::Machine(other, value),
        }
    }

    /// The type and value of a well-formed machine integer literal, `U8`
    /// included. A `Machine` literal out of range, or at `MachineInt::U8`,
    /// is not a term and has no value.
    pub fn machine_value(&self) -> Option<(MachineInt, Integer)> {
        match self {
            Self::U8(byte) => Some((MachineInt::U8, Integer::from(i128::from(*byte)))),
            Self::Machine(ty, value) if *ty != MachineInt::U8 && ty.contains(value) => {
                Some((*ty, value.clone()))
            }
            _ => None,
        }
    }

    /// `view(T)(x)`: the value of a machine integer as an `Int`.
    pub fn view(ty: MachineInt, value: Term) -> Self {
        Self::Prim(Prim::View(ty), vec![value])
    }

    /// `wrap(T)(n)`: an `Int` reduced into the range of `T`.
    pub fn wrap(ty: MachineInt, number: Term) -> Self {
        Self::Prim(Prim::Wrap(ty), vec![number])
    }

    /// `cast(S, T)(x)`: `x as T` for `x : S`.
    pub fn cast(from: MachineInt, to: MachineInt, value: Term) -> Self {
        Self::Prim(Prim::Cast(from, to), vec![value])
    }

    /// `op[T](operands)`: a row of the table of primitive operations
    /// applied. The number of operands is checked by typing, not here.
    pub fn op(op: Op, ty: MachineInt, operands: Vec<Term>) -> Self {
        Self::Prim(Prim::Op(op, ty), operands)
    }

    /// A tuple value of the given tuple type.
    pub fn tuple(ty: &Type, values: Vec<Term>) -> Self {
        match ty {
            Type::Tuple(fields) => Self::Tuple(fields.clone(), values),
            _ => panic!("Term::tuple needs a tuple type"),
        }
    }

    pub fn proj(target: Term, index: usize) -> Self {
        Self::Proj(Box::new(target), index)
    }

    pub fn proof(proof: Proof) -> Self {
        Self::Proof(Box::new(proof))
    }

    pub fn call(callee: Term, arguments: Vec<Term>) -> Self {
        Self::Call(Box::new(callee), arguments)
    }

    /// Builds a `for`. `state(i)` is the state's tuple type at index `i`;
    /// `body(i, s, lower, upper)` receives the index, the current state, and
    /// proofs of `lo <= i` and `i < hi`, and returns the next state.
    pub fn for_range(
        lo: Term,
        hi: Term,
        ordered: Proof,
        state: impl FnOnce(Term) -> Type,
        init: Term,
        body: impl FnOnce(Term, Term, Proof, Proof) -> Term,
    ) -> Self {
        let index = VarId::fresh();
        let state = match state(Self::Free(index)).close_over(&[index]) {
            Type::Tuple(fields) => fields,
            _ => panic!("the state of a for is a tuple type"),
        };
        let (i, s) = (VarId::fresh(), VarId::fresh());
        let (lower, upper) = (HypId::fresh(), HypId::fresh());
        let body = body(
            Self::Free(i),
            Self::Free(s),
            Proof::hyp(lower),
            Proof::hyp(upper),
        )
        .close_over(&[i, s])
        .rebind(Depth::default().under_hyps(1), Rebind::CloseHyp(lower))
        .rebind(Depth::default(), Rebind::CloseHyp(upper));
        Self::For(Box::new(ForLoop {
            lo,
            hi,
            ordered,
            state,
            init,
            body,
        }))
    }

    /// As `open_hyps`, replacing each hypothesis binder by a proof.
    pub(super) fn subst_hyps(&self, proofs: &[&Proof]) -> Term {
        let mut term = self.clone();
        for (j, replacement) in proofs.iter().enumerate() {
            term = term.rebind(
                Depth::default(),
                Rebind::SubstHyp {
                    index: (proofs.len() - 1 - j) as u32,
                    replacement,
                },
            );
        }
        term
    }

    /// Instantiates the hypotheses a term is under: binder `j` of
    /// `hyps.len()` is replaced by `hyps[j]`.
    pub(super) fn open_hyps(&self, hyps: &[HypId]) -> Term {
        let mut term = self.clone();
        for (j, id) in hyps.iter().enumerate() {
            term = term.rebind(
                Depth::default(),
                Rebind::OpenHyp {
                    index: (hyps.len() - 1 - j) as u32,
                    id: *id,
                },
            );
        }
        term
    }

    /// The term with every occurrence of the context variable replaced.
    pub fn replace_var(&self, var: VarId, replacement: &Term) -> Term {
        self.close(var).open(replacement)
    }

    /// The term with every use of the context hypothesis replaced by a
    /// proof of the same proposition.
    pub fn replace_hyp(&self, hyp: HypId, replacement: &Proof) -> Term {
        self.rebind(Depth::default(), Rebind::CloseHyp(hyp))
            .subst_hyps(&[replacement])
    }

    /// A case whose arms were written against identities the caller chose:
    /// each arm gives its payload variables, the identity of its fact, and
    /// its body.
    pub fn case_with(scrutinee: Term, result: Type, arms: Vec<(Vec<VarId>, HypId, Term)>) -> Self {
        let arms = arms
            .into_iter()
            .map(|(vars, fact, body)| TermArm {
                binders: vars.len() as u32,
                body: body
                    .close_over(&vars)
                    .rebind(Depth::default(), Rebind::CloseHyp(fact)),
            })
            .collect();
        Self::Case {
            scrutinee: Box::new(scrutinee),
            result,
            arms,
        }
    }

    /// A `for` written against identities the caller chose. `state` lists
    /// the state variables with their types, which may mention `index` and
    /// the state variables before them. `next` gives the state for the
    /// following index, as one term per state variable, and may mention the
    /// index, the state variables, and the two facts.
    #[allow(clippy::too_many_arguments)]
    pub fn for_with(
        index: VarId,
        lower: HypId,
        upper: HypId,
        lo: Term,
        hi: Term,
        ordered: Proof,
        state: &[(VarId, Type)],
        init: Vec<Term>,
        next: Vec<Term>,
    ) -> Self {
        let Type::Tuple(telescope) = Type::tuple_over(state) else {
            unreachable!("tuple_over builds a tuple type")
        };
        let at = |i: &Term| -> Vec<Type> {
            match Type::Tuple(telescope.clone()).replace_var(index, i) {
                Type::Tuple(fields) => fields,
                _ => unreachable!("replacing a variable preserves the shape of a type"),
            }
        };
        let i = Self::Free(index);
        let successor = Self::wrapping_add(i.clone(), Self::U8(1));
        // The body sees the state as one tuple, so each state variable
        // becomes a projection from it.
        let whole = VarId::fresh();
        let mut body = Self::Tuple(at(&successor), next);
        for (position, (var, _)) in state.iter().enumerate() {
            body = body.replace_var(*var, &Self::proj(Self::Free(whole), position));
        }
        let body = body
            .close_over(&[index, whole])
            .rebind(Depth::default().under_hyps(1), Rebind::CloseHyp(lower))
            .rebind(Depth::default(), Rebind::CloseHyp(upper));
        let Type::Tuple(state_fields) = Type::Tuple(telescope.clone()).close_over(&[index]) else {
            unreachable!("closing preserves the shape of a type")
        };
        Self::For(Box::new(ForLoop {
            init: Self::Tuple(at(&lo), init),
            lo,
            hi,
            ordered,
            state: state_fields,
            body,
        }))
    }

    /// Builds `exists (x: ty) { body(x) }`.
    pub fn exists(ty: Type, body: impl FnOnce(Term) -> Term) -> Self {
        let var = VarId::fresh();
        let body = body(Self::Free(var)).close(var);
        Self::Exists(ty, Box::new(body))
    }

    /// Builds a case. Each arm is given its payload arity and receives that
    /// many payload variables, and the proof of its arm fact.
    pub fn case(scrutinee: Term, result: Type, arms: Vec<(usize, ArmBuilder<'_>)>) -> Self {
        let arms = arms
            .into_iter()
            .map(|(arity, body)| {
                let vars: Vec<VarId> = (0..arity).map(|_| VarId::fresh()).collect();
                let payload: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
                let fact = HypId::fresh();
                TermArm {
                    binders: arity as u32,
                    body: body(&payload, Proof::hyp(fact))
                        .close_over(&vars)
                        .rebind(Depth::default(), Rebind::CloseHyp(fact)),
                }
            })
            .collect();
        Self::Case {
            scrutinee: Box::new(scrutinee),
            result,
            arms,
        }
    }

    /// Puts a term under binders for `vars`, as `Type::close_over`.
    pub(super) fn close_over(&self, vars: &[VarId]) -> Term {
        let count = vars.len() as u32;
        let mut term = self.clone();
        for (j, var) in vars.iter().enumerate() {
            term = term.rebind(Depth::at(count - 1 - j as u32), Rebind::CloseVar(*var));
        }
        term
    }

    /// Instantiates a term that is under `count` binders: binder `j` is
    /// replaced by `value(j)`, which must be locally closed.
    pub(super) fn instantiate(&self, count: usize, value: impl Fn(usize) -> Term) -> Term {
        let mut term = self.clone();
        for j in 0..count {
            let replacement = value(j);
            term = term.rebind(
                Depth::default(),
                Rebind::OpenVar {
                    index: (count - 1 - j) as u32,
                    replacement: &replacement,
                },
            );
        }
        term
    }

    /// Whether the term has no dangling bound index. Types and proofs inside
    /// it are not inspected; this is used only to choose rewrite targets.
    pub fn is_closed(&self) -> bool {
        self.closed_at(0)
    }

    fn closed_at(&self, depth: u32) -> bool {
        let all = |terms: &[Term]| terms.iter().all(|term| term.closed_at(depth));
        match self {
            Self::Bound(index) => *index < depth,
            Self::Free(_)
            | Self::Bool(_)
            | Self::U8(_)
            | Self::Nat(_)
            | Self::Int(_)
            | Self::Machine(..)
            | Self::Proof(_)
            | Self::Fn(_) => true,
            Self::Prim(_, arguments) => all(arguments),
            Self::Eq(_, left, right) => left.closed_at(depth) && right.closed_at(depth),
            Self::Implies(premise, conclusion) => {
                premise.closed_at(depth) && conclusion.closed_at(depth)
            }
            Self::Forall(_, body) => body.closed_at(depth + 1),
            Self::Tuple(_, values) | Self::Struct(_, values) => all(values),
            Self::Proj(target, _) => target.closed_at(depth),
            Self::Call(callee, arguments) => callee.closed_at(depth) && all(arguments),
            Self::Variant(_, _, payload) => all(payload),
            Self::Case {
                scrutinee, arms, ..
            } => {
                scrutinee.closed_at(depth)
                    && arms
                        .iter()
                        .all(|arm| arm.body.closed_at(depth + arm.binders))
            }
            Self::PropApp(_, arguments) => all(arguments),
            Self::Exists(_, body) => body.closed_at(depth + 1),
            Self::Absurd(_, _) => true,
            Self::For(looped) => {
                looped.lo.closed_at(depth)
                    && looped.hi.closed_at(depth)
                    && looped.init.closed_at(depth)
                    && looped.body.closed_at(depth + 2)
            }
        }
    }

    /// The first subterm, outermost and leftmost, that satisfies `wanted`.
    /// Types and proofs inside the term are not searched.
    pub fn find(&self, wanted: &impl Fn(&Term) -> bool) -> Option<&Term> {
        fn first<'a>(terms: &'a [Term], wanted: &impl Fn(&Term) -> bool) -> Option<&'a Term> {
            terms.iter().find_map(|term| term.find(wanted))
        }
        if wanted(self) {
            return Some(self);
        }
        match self {
            Self::Free(_)
            | Self::Bound(_)
            | Self::Bool(_)
            | Self::U8(_)
            | Self::Nat(_)
            | Self::Int(_)
            | Self::Machine(..)
            | Self::Proof(_)
            | Self::Fn(_)
            | Self::Absurd(_, _) => None,
            Self::Variant(_, _, payload) => first(payload, wanted),
            Self::Case {
                scrutinee, arms, ..
            } => scrutinee
                .find(wanted)
                .or_else(|| arms.iter().find_map(|arm| arm.body.find(wanted))),
            Self::PropApp(_, arguments) => first(arguments, wanted),
            Self::Exists(_, body) => body.find(wanted),
            Self::For(looped) => looped
                .lo
                .find(wanted)
                .or_else(|| looped.hi.find(wanted))
                .or_else(|| looped.init.find(wanted))
                .or_else(|| looped.body.find(wanted)),
            Self::Prim(_, arguments) => first(arguments, wanted),
            Self::Eq(_, left, right) => left.find(wanted).or_else(|| right.find(wanted)),
            Self::Implies(premise, conclusion) => {
                premise.find(wanted).or_else(|| conclusion.find(wanted))
            }
            Self::Forall(_, body) => body.find(wanted),
            Self::Tuple(_, values) | Self::Struct(_, values) => first(values, wanted),
            Self::Proj(target, _) => target.find(wanted),
            Self::Call(callee, arguments) => {
                callee.find(wanted).or_else(|| first(arguments, wanted))
            }
        }
    }

    /// A template whose hole stands for every occurrence of `target` that
    /// `is_target` recognizes. `target` must be locally closed. Occurrences
    /// inside types and proofs are left alone, which keeps the template
    /// valid: opening it with `target` gives back this term.
    pub fn abstract_over(&self, is_target: &impl Fn(&Term) -> bool) -> Term {
        self.abstract_at(is_target, 0)
    }

    fn abstract_at(&self, is_target: &impl Fn(&Term) -> bool, depth: u32) -> Term {
        if is_target(self) {
            return Self::Bound(depth);
        }
        let each = |terms: &[Term]| -> Vec<Term> {
            terms
                .iter()
                .map(|term| term.abstract_at(is_target, depth))
                .collect()
        };
        let boxed = |term: &Term, depth: u32| Box::new(term.abstract_at(is_target, depth));
        match self {
            Self::Free(_)
            | Self::Bound(_)
            | Self::Bool(_)
            | Self::U8(_)
            | Self::Nat(_)
            | Self::Int(_)
            | Self::Machine(..)
            | Self::Proof(_)
            | Self::Fn(_)
            | Self::Absurd(_, _) => self.clone(),
            Self::Variant(id, index, payload) => Self::Variant(*id, *index, each(payload)),
            Self::Case {
                scrutinee,
                result,
                arms,
            } => Self::Case {
                scrutinee: boxed(scrutinee, depth),
                result: result.clone(),
                arms: arms
                    .iter()
                    .map(|arm| TermArm {
                        binders: arm.binders,
                        body: arm.body.abstract_at(is_target, depth + arm.binders),
                    })
                    .collect(),
            },
            Self::PropApp(id, arguments) => Self::PropApp(*id, each(arguments)),
            Self::Exists(ty, body) => Self::Exists(ty.clone(), boxed(body, depth + 1)),
            Self::For(looped) => Self::For(Box::new(ForLoop {
                lo: looped.lo.abstract_at(is_target, depth),
                hi: looped.hi.abstract_at(is_target, depth),
                ordered: looped.ordered.clone(),
                state: looped.state.clone(),
                init: looped.init.abstract_at(is_target, depth),
                body: looped.body.abstract_at(is_target, depth + 2),
            })),
            Self::Prim(prim, arguments) => Self::Prim(*prim, each(arguments)),
            Self::Eq(ty, left, right) => {
                Self::Eq(ty.clone(), boxed(left, depth), boxed(right, depth))
            }
            Self::Implies(premise, conclusion) => {
                Self::Implies(boxed(premise, depth), boxed(conclusion, depth))
            }
            Self::Forall(ty, body) => Self::Forall(ty.clone(), boxed(body, depth + 1)),
            Self::Tuple(fields, values) => Self::Tuple(fields.clone(), each(values)),
            Self::Struct(id, values) => Self::Struct(*id, each(values)),
            Self::Proj(target, index) => Self::Proj(boxed(target, depth), *index),
            Self::Call(callee, arguments) => Self::Call(boxed(callee, depth), each(arguments)),
        }
    }

    /// Replaces the outermost bound variable with a locally closed term.
    pub fn open(&self, replacement: &Term) -> Term {
        self.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: 0,
                replacement,
            },
        )
    }

    /// Turns a context variable into the outermost bound variable.
    pub(super) fn close(&self, var: VarId) -> Term {
        self.rebind(Depth::default(), Rebind::CloseVar(var))
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Term {
        match self {
            Self::Free(..) => self.rebind_free(depth, op),
            Self::Bound(..) => self.rebind_bound(depth, op),
            Self::Bool(_) | Self::U8(_) | Self::Nat(_) | Self::Int(_) | Self::Machine(..) => {
                self.clone()
            }
            Self::Prim(..) => self.rebind_prim(depth, op),
            Self::Eq(..) => self.rebind_eq(depth, op),
            Self::Implies(..) => self.rebind_implies(depth, op),
            Self::Forall(..) => self.rebind_forall(depth, op),
            Self::Tuple(..) => self.rebind_tuple(depth, op),
            Self::Struct(..) => self.rebind_struct(depth, op),
            Self::Proj(..) => self.rebind_proj(depth, op),
            Self::Proof(..) => self.rebind_proof(depth, op),
            Self::Fn(_) => self.clone(),
            Self::Call(..) => self.rebind_call(depth, op),
            Self::Variant(..) => self.rebind_variant(depth, op),
            Self::Case { .. } => self.rebind_case(depth, op),
            Self::PropApp(..) => self.rebind_prop_app(depth, op),
            Self::Exists(..) => self.rebind_exists(depth, op),
            Self::Absurd(..) => self.rebind_absurd(depth, op),
            Self::For(..) => self.rebind_for(depth, op),
        }
    }

    #[inline(never)]
    fn rebind_free(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Free(id) = self else {
            unreachable!("dispatched on this variant")
        };
        match op {
            Rebind::CloseVar(var) if var == *id => Self::Bound(depth.vars),
            _ => self.clone(),
        }
    }

    #[inline(never)]
    fn rebind_bound(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Bound(bound) = self else {
            unreachable!("dispatched on this variant")
        };
        match op {
            Rebind::OpenVar { index, replacement } if *bound == depth.vars + index => {
                replacement.clone()
            }
            _ => self.clone(),
        }
    }

    #[inline(never)]
    fn rebind_prim(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Prim(prim, arguments) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Prim(*prim, rebind_each(depth, op, arguments))
    }

    #[inline(never)]
    fn rebind_eq(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Eq(ty, left, right) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Eq(
            ty.rebind(depth, op),
            Box::new(left.rebind(depth, op)),
            Box::new(right.rebind(depth, op)),
        )
    }

    #[inline(never)]
    fn rebind_implies(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Implies(premise, conclusion) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Implies(
            Box::new(premise.rebind(depth, op)),
            Box::new(conclusion.rebind(depth, op)),
        )
    }

    #[inline(never)]
    fn rebind_forall(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Forall(ty, body) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Forall(
            ty.rebind(depth, op),
            Box::new(body.rebind(depth.under_vars(1), op)),
        )
    }

    #[inline(never)]
    fn rebind_tuple(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Tuple(fields, values) = self else {
            unreachable!("dispatched on this variant")
        };
        {
            let Type::Tuple(fields) = Type::Tuple(fields.clone()).rebind(depth, op) else {
                unreachable!("rebinding preserves the shape of a type")
            };
            Self::Tuple(fields, rebind_each(depth, op, values))
        }
    }

    #[inline(never)]
    fn rebind_struct(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Struct(id, values) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Struct(*id, rebind_each(depth, op, values))
    }

    #[inline(never)]
    fn rebind_proj(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Proj(target, index) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Proj(Box::new(target.rebind(depth, op)), *index)
    }

    #[inline(never)]
    fn rebind_proof(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Proof(proof) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Proof(Box::new(proof.rebind(depth, op)))
    }

    #[inline(never)]
    fn rebind_call(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Call(callee, arguments) = self else {
            unreachable!("dispatched on this variant")
        };
        {
            Self::Call(
                Box::new(callee.rebind(depth, op)),
                rebind_each(depth, op, arguments),
            )
        }
    }

    #[inline(never)]
    fn rebind_variant(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Variant(id, index, payload) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Variant(*id, *index, rebind_each(depth, op, payload))
    }

    #[inline(never)]
    fn rebind_case(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Case {
            scrutinee,
            result,
            arms,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::Case {
            scrutinee: Box::new(scrutinee.rebind(depth, op)),
            result: result.rebind(depth, op),
            arms: arms
                .iter()
                .map(|arm| TermArm {
                    binders: arm.binders,
                    body: arm.body.rebind(depth.under(arm.binders, 1), op),
                })
                .collect(),
        }
    }

    #[inline(never)]
    fn rebind_prop_app(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::PropApp(id, arguments) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::PropApp(*id, rebind_each(depth, op, arguments))
    }

    #[inline(never)]
    fn rebind_exists(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Exists(ty, body) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Exists(
            ty.rebind(depth, op),
            Box::new(body.rebind(depth.under_vars(1), op)),
        )
    }

    #[inline(never)]
    fn rebind_absurd(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::Absurd(proof, ty) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Absurd(Box::new(proof.rebind(depth, op)), ty.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_for(&self, depth: Depth, op: Rebind<'_>) -> Term {
        let Self::For(looped) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::For(Box::new(ForLoop {
            lo: looped.lo.rebind(depth, op),
            hi: looped.hi.rebind(depth, op),
            ordered: looped.ordered.rebind(depth, op),
            state: rebind_telescope(&looped.state, depth.under_vars(1), op),
            init: looped.init.rebind(depth, op),
            body: looped.body.rebind(depth.under(2, 2), op),
        }))
    }
}

fn rebind_each(depth: Depth, op: Rebind<'_>, terms: &[Term]) -> Vec<Term> {
    terms.iter().map(|term| term.rebind(depth, op)).collect()
}

/// The type of field `index` of a telescope, with each earlier field `j`
/// replaced by `earlier(j)`. The replacements must be locally closed.
pub(super) fn field_type(fields: &[Type], index: usize, earlier: impl Fn(usize) -> Term) -> Type {
    let mut ty = fields[index].clone();
    for j in 0..index {
        let replacement = earlier(j);
        ty = ty.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: (index - 1 - j) as u32,
                replacement: &replacement,
            },
        );
    }
    ty
}

impl Proof {
    /// The name the kernel contract gives the rule.
    pub fn rule_name(&self) -> &'static str {
        match self {
            Self::Hyp(_) => "hyp",
            Self::OfTerm(_) => "of_term",
            Self::Refl(_) => "refl",
            Self::Transport { .. } => "transport",
            Self::ImpliesIntro { .. } => "implies_intro",
            Self::ImpliesElim(..) => "implies_elim",
            Self::ForallIntro { .. } => "forall_intro",
            Self::ForallElim(..) => "forall_elim",
            Self::Projection(_) => "projection",
            Self::Literal(_) => "literal",
            Self::Definition(_) => "definition",
            Self::CaseStep(_) => "case_step",
            Self::Construct { .. } => "construct",
            Self::CaseProof { .. } => "case_proof",
            Self::CaseData { .. } => "case_data",
            Self::ExistsIntro { .. } => "exists_intro",
            Self::ExistsElim { .. } => "exists_elim",
            Self::ExcludedMiddle(_) => "excluded_middle",
            Self::ForEmpty(_) => "for_empty",
            Self::ForStep { .. } => "for_step",
            Self::Omitted => "omitted",
            Self::Evaluate(_) => "evaluate",
            Self::EvaluateAll(_) => "evaluate_all",
            Self::Axiom(_) => "axiom",
            Self::NatInduction { .. } => "nat_induction",
            Self::IntInduction { .. } => "int_induction",
            Self::Linear { .. } => "linear",
        }
    }

    pub fn hyp(id: HypId) -> Self {
        Self::Hyp(HypRef::Free(id))
    }

    /// Builds a transport whose template is `template(hole)`.
    pub fn transport(eq: Proof, template: impl FnOnce(Term) -> Term, proof: Proof) -> Self {
        let hole = VarId::fresh();
        Self::Transport {
            eq: Box::new(eq),
            template: template(Term::Free(hole)).close(hole),
            proof: Box::new(proof),
        }
    }

    /// Builds a proof of `hyp => Q` from a proof of `Q` that may use `hyp`.
    pub fn implies_intro(hyp: Term, body: impl FnOnce(Proof) -> Proof) -> Self {
        let id = HypId::fresh();
        let body = body(Self::hyp(id)).rebind(Depth::default(), Rebind::CloseHyp(id));
        Self::ImpliesIntro {
            hyp,
            body: Box::new(body),
        }
    }

    pub fn implies_elim(implication: Proof, premise: Proof) -> Self {
        Self::ImpliesElim(Box::new(implication), Box::new(premise))
    }

    /// Builds a proof of `forall (x: ty) { P(x) }` from a proof of `P(x)`.
    pub fn forall_intro(ty: Type, body: impl FnOnce(Term) -> Proof) -> Self {
        let var = VarId::fresh();
        let body = body(Term::Free(var)).rebind(Depth::default(), Rebind::CloseVar(var));
        Self::ForallIntro {
            ty,
            body: Box::new(body),
        }
    }

    /// Builds an induction. `motive(n)` is the claim about `n`; `step(n, ih)`
    /// proves the claim about `succ(n)` from `ih`, the claim about `n`.
    pub fn nat_induction(
        motive: impl FnOnce(Term) -> Term,
        base: Proof,
        step: impl FnOnce(Term, Proof) -> Proof,
        target: Term,
    ) -> Self {
        let hole = VarId::fresh();
        Self::NatInduction {
            motive: motive(Term::Free(hole)).close(hole),
            base: Box::new(base),
            step: Self::arm(1, 1, |vars, hyps| step(vars[0].clone(), hyps[0].clone())),
            target,
        }
    }

    /// Builds an induction over the non-negative integers. `motive(n)` is
    /// the claim about `n`; `step(n, nonneg, ih)` proves the claim about
    /// `n + 1` from `nonneg`, that `0 <= n`, and `ih`, the claim about `n`.
    pub fn int_induction(
        motive: impl FnOnce(Term) -> Term,
        base: Proof,
        step: impl FnOnce(Term, Proof, Proof) -> Proof,
        target: Term,
    ) -> Self {
        let hole = VarId::fresh();
        Self::IntInduction {
            motive: motive(Term::Free(hole)).close(hole),
            base: Box::new(base),
            step: Self::arm(1, 2, |vars, hyps| {
                step(vars[0].clone(), hyps[0].clone(), hyps[1].clone())
            }),
            target,
        }
    }

    /// Builds a linear certificate for `goal` from pairs of a proof and
    /// its coefficient. The goal's coefficient is `goal_coefficient`.
    pub fn linear(goal: Term, goal_coefficient: i64, pairs: Vec<(Proof, i64)>) -> Self {
        Self::Linear {
            goal,
            goal_coefficient: Integer::from(goal_coefficient),
            pairs: pairs
                .into_iter()
                .map(|(proof, coefficient)| (proof, Integer::from(coefficient)))
                .collect(),
        }
    }

    /// Builds `forall (x: u8) { body(x) == true }` by evaluation.
    pub fn evaluate_all(body: impl FnOnce(Term) -> Term) -> Self {
        let var = VarId::fresh();
        Self::EvaluateAll(body(Term::Free(var)).close(var))
    }

    pub fn forall_elim(universal: Proof, argument: Term) -> Self {
        Self::ForallElim(Box::new(universal), argument)
    }

    /// Turns a context variable into the outermost bound term variable.
    pub(super) fn close_var(&self, var: VarId) -> Proof {
        self.rebind(Depth::default(), Rebind::CloseVar(var))
    }

    /// Turns a context hypothesis into the outermost bound hypothesis.
    pub(super) fn close_hyp(&self, id: HypId) -> Proof {
        self.rebind(Depth::default(), Rebind::CloseHyp(id))
    }

    pub(super) fn open_var(&self, replacement: &Term) -> Proof {
        self.rebind(
            Depth::default(),
            Rebind::OpenVar {
                index: 0,
                replacement,
            },
        )
    }

    pub(super) fn open_hyp(&self, id: HypId) -> Proof {
        self.rebind(Depth::default(), Rebind::OpenHyp { index: 0, id })
    }

    /// Instantiates an arm body that is under `vars.len()` term binders and
    /// `hyps.len()` hypothesis binders.
    pub(super) fn open_arm(&self, vars: &[VarId], hyps: &[HypId]) -> Proof {
        let mut proof = self.clone();
        for (j, var) in vars.iter().enumerate() {
            proof = proof.rebind(
                Depth::default(),
                Rebind::OpenVar {
                    index: (vars.len() - 1 - j) as u32,
                    replacement: &Term::Free(*var),
                },
            );
        }
        for (j, id) in hyps.iter().enumerate() {
            proof = proof.rebind(
                Depth::default(),
                Rebind::OpenHyp {
                    index: (hyps.len() - 1 - j) as u32,
                    id: *id,
                },
            );
        }
        proof
    }

    /// Builds an arm whose body receives `vars` payload variables and `hyps`
    /// hypotheses.
    pub fn arm(
        vars: usize,
        hyps: usize,
        body: impl FnOnce(&[Term], &[Proof]) -> Proof,
    ) -> ProofArm {
        let var_ids: Vec<VarId> = (0..vars).map(|_| VarId::fresh()).collect();
        let hyp_ids: Vec<HypId> = (0..hyps).map(|_| HypId::fresh()).collect();
        let terms: Vec<Term> = var_ids.iter().copied().map(Term::Free).collect();
        let proofs: Vec<Proof> = hyp_ids.iter().copied().map(Proof::hyp).collect();
        let mut proof = body(&terms, &proofs);
        for (j, var) in var_ids.iter().enumerate() {
            let depth = Depth::at((vars - 1 - j) as u32);
            proof = proof.rebind(depth, Rebind::CloseVar(*var));
        }
        for (j, id) in hyp_ids.iter().enumerate() {
            let depth = Depth::default().under_hyps((hyps - 1 - j) as u32);
            proof = proof.rebind(depth, Rebind::CloseHyp(*id));
        }
        ProofArm {
            vars: vars as u32,
            hyps: hyps as u32,
            body: Box::new(proof),
        }
    }

    pub(super) fn rebind(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        match self {
            Self::Hyp(HypRef::Free(_)) => self.rebind_hyp(depth, op),
            Self::Hyp(HypRef::Bound(_)) => self.rebind_hyp_2(depth, op),
            Self::OfTerm(..) => self.rebind_of_term(depth, op),
            Self::Refl(..) => self.rebind_refl(depth, op),
            Self::Transport { .. } => self.rebind_transport(depth, op),
            Self::ImpliesIntro { .. } => self.rebind_implies_intro(depth, op),
            Self::ImpliesElim(..) => self.rebind_implies_elim(depth, op),
            Self::ForallIntro { .. } => self.rebind_forall_intro(depth, op),
            Self::ForallElim(..) => self.rebind_forall_elim(depth, op),
            Self::Projection(..) => self.rebind_projection(depth, op),
            Self::Literal(..) => self.rebind_literal(depth, op),
            Self::Definition(..) => self.rebind_definition(depth, op),
            Self::CaseStep(..) => self.rebind_case_step(depth, op),
            Self::Construct { .. } => self.rebind_construct(depth, op),
            Self::CaseProof { .. } => self.rebind_case_proof(depth, op),
            Self::CaseData { .. } => self.rebind_case_data(depth, op),
            Self::ExistsIntro { .. } => self.rebind_exists_intro(depth, op),
            Self::ExistsElim { .. } => self.rebind_exists_elim(depth, op),
            Self::ExcludedMiddle(..) => self.rebind_excluded_middle(depth, op),
            Self::ForEmpty(..) => self.rebind_for_empty(depth, op),
            Self::ForStep { .. } => self.rebind_for_step(depth, op),
            Self::Omitted => Self::Omitted,
            Self::Evaluate(..) => self.rebind_evaluate(depth, op),
            Self::EvaluateAll(..) => self.rebind_evaluate_all(depth, op),
            Self::Axiom(..) => self.rebind_axiom(depth, op),
            Self::NatInduction { .. } => self.rebind_nat_induction(depth, op),
            Self::IntInduction { .. } => self.rebind_int_induction(depth, op),
            Self::Linear { .. } => self.rebind_linear(depth, op),
        }
    }

    #[inline(never)]
    fn rebind_hyp(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Hyp(HypRef::Free(id)) = self else {
            unreachable!("dispatched on this variant")
        };
        match op {
            Rebind::CloseHyp(target) if target == *id => Self::Hyp(HypRef::Bound(depth.hyps)),
            _ => self.clone(),
        }
    }

    #[inline(never)]
    fn rebind_hyp_2(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Hyp(HypRef::Bound(bound)) = self else {
            unreachable!("dispatched on this variant")
        };
        match op {
            Rebind::OpenHyp { index, id } if *bound == depth.hyps + index => {
                Self::Hyp(HypRef::Free(id))
            }
            Rebind::SubstHyp { index, replacement } if *bound == depth.hyps + index => {
                replacement.clone()
            }
            _ => self.clone(),
        }
    }

    #[inline(never)]
    fn rebind_of_term(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::OfTerm(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::OfTerm(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_refl(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Refl(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Refl(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_transport(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Transport {
            eq,
            template,
            proof,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::Transport {
            eq: Box::new(eq.rebind(depth, op)),
            template: template.rebind(depth.under_vars(1), op),
            proof: Box::new(proof.rebind(depth, op)),
        }
    }

    #[inline(never)]
    fn rebind_implies_intro(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ImpliesIntro { hyp, body } = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ImpliesIntro {
            hyp: hyp.rebind(depth, op),
            body: Box::new(body.rebind(depth.under_hyp(), op)),
        }
    }

    #[inline(never)]
    fn rebind_implies_elim(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ImpliesElim(implication, premise) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ImpliesElim(
            Box::new(implication.rebind(depth, op)),
            Box::new(premise.rebind(depth, op)),
        )
    }

    #[inline(never)]
    fn rebind_forall_intro(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ForallIntro { ty, body } = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ForallIntro {
            ty: ty.rebind(depth, op),
            body: Box::new(body.rebind(depth.under_vars(1), op)),
        }
    }

    #[inline(never)]
    fn rebind_forall_elim(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ForallElim(universal, argument) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ForallElim(
            Box::new(universal.rebind(depth, op)),
            argument.rebind(depth, op),
        )
    }

    #[inline(never)]
    fn rebind_projection(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Projection(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Projection(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_literal(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Literal(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Literal(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_definition(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Definition(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Definition(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_case_step(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::CaseStep(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::CaseStep(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_construct(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Construct {
            prop,
            variant,
            params,
            payload,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::Construct {
            prop: *prop,
            variant: *variant,
            params: params.iter().map(|term| term.rebind(depth, op)).collect(),
            payload: payload.iter().map(|term| term.rebind(depth, op)).collect(),
        }
    }

    #[inline(never)]
    fn rebind_case_proof(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::CaseProof {
            scrutinee,
            goal,
            arms,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::CaseProof {
            scrutinee: Box::new(scrutinee.rebind(depth, op)),
            goal: goal.rebind(depth, op),
            arms: arms.iter().map(|arm| arm.rebind(depth, op)).collect(),
        }
    }

    #[inline(never)]
    fn rebind_case_data(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::CaseData {
            scrutinee,
            goal,
            arms,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::CaseData {
            scrutinee: scrutinee.rebind(depth, op),
            goal: goal.rebind(depth, op),
            arms: arms.iter().map(|arm| arm.rebind(depth, op)).collect(),
        }
    }

    #[inline(never)]
    fn rebind_exists_intro(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ExistsIntro {
            prop,
            witness,
            proof,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::ExistsIntro {
            prop: prop.rebind(depth, op),
            witness: witness.rebind(depth, op),
            proof: Box::new(proof.rebind(depth, op)),
        }
    }

    #[inline(never)]
    fn rebind_exists_elim(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ExistsElim { exists, goal, arm } = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ExistsElim {
            exists: Box::new(exists.rebind(depth, op)),
            goal: goal.rebind(depth, op),
            arm: arm.rebind(depth, op),
        }
    }

    #[inline(never)]
    fn rebind_excluded_middle(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ExcludedMiddle(prop) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ExcludedMiddle(prop.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_for_empty(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ForEmpty(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::ForEmpty(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_for_step(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::ForStep {
            looped,
            lower,
            upper,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::ForStep {
            looped: looped.rebind(depth, op),
            lower: Box::new(lower.rebind(depth, op)),
            upper: Box::new(upper.rebind(depth, op)),
        }
    }

    #[inline(never)]
    fn rebind_evaluate(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Evaluate(term) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Evaluate(term.rebind(depth, op))
    }

    #[inline(never)]
    fn rebind_evaluate_all(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::EvaluateAll(body) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::EvaluateAll(body.rebind(depth.under_vars(1), op))
    }

    #[inline(never)]
    fn rebind_axiom(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Axiom(axiom) = self else {
            unreachable!("dispatched on this variant")
        };
        Self::Axiom(axiom.map(|term| term.rebind(depth, op)))
    }

    #[inline(never)]
    fn rebind_nat_induction(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::NatInduction {
            motive,
            base,
            step,
            target,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::NatInduction {
            motive: motive.rebind(depth.under_vars(1), op),
            base: Box::new(base.rebind(depth, op)),
            step: step.rebind(depth, op),
            target: target.rebind(depth, op),
        }
    }

    #[inline(never)]
    fn rebind_int_induction(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::IntInduction {
            motive,
            base,
            step,
            target,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::IntInduction {
            motive: motive.rebind(depth.under_vars(1), op),
            base: Box::new(base.rebind(depth, op)),
            step: step.rebind(depth, op),
            target: target.rebind(depth, op),
        }
    }

    #[inline(never)]
    fn rebind_linear(&self, depth: Depth, op: Rebind<'_>) -> Proof {
        let Self::Linear {
            goal,
            goal_coefficient,
            pairs,
        } = self
        else {
            unreachable!("dispatched on this variant")
        };
        Self::Linear {
            goal: goal.rebind(depth, op),
            goal_coefficient: goal_coefficient.clone(),
            pairs: pairs
                .iter()
                .map(|(proof, coefficient)| (proof.rebind(depth, op), coefficient.clone()))
                .collect(),
        }
    }
}

impl ProofArm {
    fn rebind(&self, depth: Depth, op: Rebind<'_>) -> ProofArm {
        ProofArm {
            vars: self.vars,
            hyps: self.hyps,
            body: Box::new(
                self.body
                    .rebind(depth.under_vars(self.vars).under_hyps(self.hyps), op),
            ),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => f.write_str("bool"),
            Self::U8 => f.write_str("u8"),
            Self::Nat => f.write_str("Nat"),
            Self::Int => f.write_str("Int"),
            Self::Machine(ty) => f.write_str(ty.name()),
            Self::Prop => f.write_str("Prop"),
            Self::Proof(prop) => write!(f, "@{prop}"),
            Self::Tuple(fields) => {
                f.write_str("(")?;
                for field in fields {
                    write!(f, "{field}, ")?;
                }
                f.write_str(")")
            }
            Self::Struct(StructId(id)) => write!(f, "struct#{id}"),
            Self::Enum(EnumId(id)) => write!(f, "enum#{id}"),
            Self::Fn(params, result) => {
                f.write_str("math fn(")?;
                for param in params {
                    write!(f, "{param}, ")?;
                }
                write!(f, ") -> {result}")
            }
        }
    }
}

fn write_list(f: &mut fmt::Formatter<'_>, terms: &[Term]) -> fmt::Result {
    for (index, term) in terms.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{term}")?;
    }
    Ok(())
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Free(VarId(id)) => write!(f, "v{id}"),
            Self::Bound(index) => write!(f, "#{index}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::U8(value) => write!(f, "{value}"),
            Self::Nat(value) => write!(f, "{value}n"),
            Self::Int(value) => write!(f, "{value}i"),
            Self::Machine(ty, value) => write!(f, "{value}{}", ty.name()),
            Self::Prim(prim, arguments) => {
                write!(f, "{prim}(")?;
                write_list(f, arguments)?;
                f.write_str(")")
            }
            Self::Eq(_, left, right) => write!(f, "({left} == {right})"),
            Self::Implies(premise, conclusion) => write!(f, "({premise} => {conclusion})"),
            Self::Forall(ty, body) => write!(f, "forall (#: {ty}) {{ {body} }}"),
            Self::Tuple(_, values) => {
                f.write_str("(")?;
                write_list(f, values)?;
                f.write_str(")")
            }
            Self::Struct(StructId(id), values) => {
                write!(f, "struct#{id} {{ ")?;
                write_list(f, values)?;
                f.write_str(" }")
            }
            Self::Proj(target, index) => write!(f, "{target}.{index}"),
            Self::Proof(_) => f.write_str("<proof>"),
            Self::Fn(FnId(id)) => write!(f, "fn#{id}"),
            Self::Call(callee, arguments) => {
                write!(f, "{callee}(")?;
                write_list(f, arguments)?;
                f.write_str(")")
            }
            Self::Variant(EnumId(id), index, payload) => {
                write!(f, "enum#{id}::{index}(")?;
                write_list(f, payload)?;
                f.write_str(")")
            }
            Self::Case {
                scrutinee, arms, ..
            } => {
                write!(f, "match {scrutinee} {{ ")?;
                for arm in arms {
                    write!(f, "{}, ", arm.body)?;
                }
                f.write_str("}")
            }
            Self::PropApp(PropId(id), arguments) => {
                write!(f, "prop#{id}(")?;
                write_list(f, arguments)?;
                f.write_str(")")
            }
            Self::Exists(ty, body) => write!(f, "exists (#: {ty}) {{ {body} }}"),
            Self::Absurd(_, ty) => write!(f, "absurd: {ty}"),
            Self::For(looped) => write!(
                f,
                "for # in {}..{} ({}) {{ {} }}",
                looped.lo, looped.hi, looped.init, looped.body
            ),
        }
    }
}
