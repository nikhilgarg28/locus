//! The typed tree.
//!
//! Its executable skeleton mirrors the source: the same nesting and names,
//! `if` as `if`, `match` as `match`, method calls as method calls. Its
//! logical content is kernel-level: types are kernel `Type`s written over the
//! identities of the binders in scope, a proof expression is a kernel
//! `Proof`, and a proposition value is a kernel `Term`. Nothing logical is
//! ever printed, so it needs no source shape.
//!
//! The nodes that stand for a place or a call carry their type (`Var`,
//! `Field`, `CallMath`, `CallFn`), and the control forms carry their result
//! type. That is enough to tell, for any expression, whether its value is a
//! proof, which both lowering and erasure need to know.
//!
//! Everything the checker will bind has an identity here, because the proofs
//! in the tree refer to those identities: binders, the fact of each branch
//! and arm, the equation of each `let`, the versions a loop's body sees of
//! what it carries, and the result of each call, `if`, `match`, and loop.

use crate::exec::ExecFnId;
use crate::kernel::{
    EnumId, FnId, HypId, Integer, MachineInt, Op, Prim, Proof, StructId, Term, Type, VarId,
};

/// A binding occurrence: an identity, the spelling to print, and its type.
///
/// `ghost` is a binding declared `Ghost<T>`: a logical value of the type
/// `ty`, which is `T`, with no runtime form. The kernel sees `T`, since it
/// has no runtime/ghost distinction beyond modes; erasure sees a marker,
/// as for evidence, and leaves a `let` of it out.
#[derive(Clone, Debug)]
pub struct Binder {
    pub id: VarId,
    pub name: String,
    pub ty: Type,
    pub ghost: bool,
}

/// A trait a struct or an enum derives, from the closed list of
/// `#[derive(...)]`. The elaborator checks that the type may derive it; the
/// printer writes the list on the generated type, in the order given, and
/// adds nothing of its own. A type without `Copy` moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Derive {
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
}

impl Derive {
    /// The closed list, in the order Rust conventionally writes it.
    pub const ALL: [Self; 5] = [
        Self::Clone,
        Self::Copy,
        Self::PartialEq,
        Self::Eq,
        Self::Debug,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Clone => "Clone",
            Self::Copy => "Copy",
            Self::PartialEq => "PartialEq",
            Self::Eq => "Eq",
            Self::Debug => "Debug",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|derive| derive.name() == name)
    }
}

/// `struct Name { field: Type, ... }`. A field's type may mention the
/// fields before it, by their binders' identities.
#[derive(Clone, Debug)]
pub struct StructItem {
    pub name: String,
    pub fields: Vec<Binder>,
    /// `#[derive(...)]`, as written and in that order.
    pub derives: Vec<Derive>,
}

#[derive(Clone, Debug)]
pub struct EnumItem {
    pub name: String,
    pub variants: Vec<VariantItem>,
    /// `#[derive(...)]`, as written and in that order.
    pub derives: Vec<Derive>,
}

#[derive(Clone, Debug)]
pub struct VariantItem {
    pub name: String,
    pub payload: Vec<Binder>,
    /// Written with braces, `V { a: T, b: U }`: the payload's binders are
    /// its field names, and Rust reads and writes them by name.
    pub named: bool,
}

/// `fn` or `math fn`. The result type may mention the parameters.
#[derive(Clone, Debug)]
pub struct FnItem {
    pub name: String,
    pub math: bool,
    pub params: Vec<Binder>,
    pub result: Type,
    pub body: Block,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Absent means the block's value is unit.
    pub tail: Option<Box<Expr>>,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let {
        pattern: Pattern,
        value: Expr,
    },
    /// `place = value;`. The place is a binding declared with `let mut`,
    /// whole or by a field path. Lowering gives the binding a new version,
    /// `version`, a `let` of the old version with the path replaced by the
    /// value, under the equation `equation`; every later mention of the
    /// binding is that version. The right side is evaluated first, and the
    /// place is rebuilt from the versions current after it.
    Assign {
        place: Place,
        value: Expr,
        version: Binder,
        equation: HypId,
    },
    Expr(Expr),
}

/// The left side of an assignment: the identity of the binding declared by
/// `let mut`, its name, and the path of fields into it, outermost first.
#[derive(Clone, Debug)]
pub struct Place {
    pub binding: VarId,
    pub name: String,
    pub path: Vec<Step>,
}

/// One field of a place's path, with what rebuilding the product around it
/// needs: the product's type, and which of its fields hold evidence, whose
/// number is the product's arity.
#[derive(Clone, Debug)]
pub struct Step {
    pub index: usize,
    pub name: Option<String>,
    pub ty: Type,
    pub proof_fields: Vec<bool>,
}

/// What an `if` or `match` carries when some arm assigns a binding declared
/// outside it. Lowering makes the branch a match whose result, `tuple`, is
/// the new versions of the assigned bindings followed by the branch's value;
/// each is then bound by a `let` of the projection: the versions under the
/// identities of `joins`, in the order lowering fixes, and the value under
/// the branch's `result`, with `equation`. Lowering computes the set of
/// assigned bindings itself and rejects a tree whose `joins` differ.
#[derive(Clone, Debug)]
pub struct Joined {
    pub tuple: VarId,
    pub joins: Vec<Join>,
    pub equation: HypId,
}

/// A binding assigned in some arm, and the version it has after the join.
#[derive(Clone, Debug)]
pub struct Join {
    pub binding: VarId,
    pub version: Binder,
    pub equation: HypId,
}

/// What a loop carries: the bindings declared outside it that it assigns,
/// in declaration order, each with the version it has after the loop, and
/// the identity of the loop's result, a tuple of those versions followed,
/// for a `loop`, by the value of the `break`. After the loop each version
/// is bound by projection, as after a branch that assigns. Lowering computes
/// the set of assigned bindings itself and rejects a tree whose `joins`
/// differ.
#[derive(Clone, Debug)]
pub struct Carried {
    pub tuple: VarId,
    pub joins: Vec<Join>,
}

#[derive(Clone, Debug)]
pub enum Pattern {
    /// A name, with the identity of the equation `name == value`. A proof
    /// has no equation and the identity is unused. `mutable` is `let mut`:
    /// the name may be assigned, and its identity is the binding every
    /// version of it refers to.
    Bind {
        binder: Binder,
        equation: HypId,
        mutable: bool,
    },
    Wildcard,
    Tuple(Vec<Pattern>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Var {
        id: VarId,
        name: String,
        ty: Type,
    },
    Bool(bool),
    /// A literal of a machine integer type, within its range. An `i128`
    /// holds every value of every type up to 64 bits.
    Literal(MachineInt, i128),
    /// A literal of `Int`, of any size. Logic-only, like every `Int`.
    Int(Integer),
    /// `ty` is the tuple's kernel type, a telescope.
    Tuple {
        ty: Type,
        fields: Vec<Expr>,
    },
    Struct {
        id: StructId,
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Variant {
        id: EnumId,
        enum_name: String,
        index: usize,
        variant_name: String,
        payload: Vec<Expr>,
    },
    /// `target.index`, or `target.name` when the field has a name.
    Field {
        target: Box<Expr>,
        index: usize,
        name: Option<String>,
        ty: Type,
    },
    /// `receiver.method(arguments)` for a primitive operation: a row of
    /// the table, `Prim::Op`, at the receiver's type.
    Method {
        prim: Prim,
        receiver: Box<Expr>,
        arguments: Vec<Expr>,
    },
    /// `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, or `-a` at the machine
    /// type `ty`: a row of the table that may panic, so not a term but a
    /// statement of the check IR, `exec::OperateStmt`, whose value is
    /// `result` under the equation `equation`, the wrapped meaning. `fits`
    /// is the evidence that it does not panic, one proof per premise of
    /// `Row::fits`, which the elaborator fills under `no_panic` and leaves
    /// out otherwise; `learned` names the facts known afterwards, as the
    /// statement says.
    Operate {
        op: Op,
        ty: MachineInt,
        operands: Vec<Expr>,
        result: VarId,
        equation: HypId,
        fits: Option<Vec<Proof>>,
        learned: Vec<HypId>,
    },
    /// The same operators on `Int`, which are total: a term of the logic,
    /// `int_add` and the rest, with no runtime form.
    IntArith {
        op: Op,
        operands: Vec<Expr>,
    },
    /// A comparison of two values of one type, `ty`, which is a machine
    /// integer type or, for `==` and `!=`, `bool`. Of type `bool`.
    Compare {
        op: CompareOp,
        ty: Type,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `expr as to`, for a value of type `from`. Between machine types it
    /// wraps and has a runtime form. `as Int` is the view and `Int as T` the
    /// wrap; both are logic-only, and stand only where nothing runs.
    Cast {
        expr: Box<Expr>,
        from: Type,
        to: Type,
    },
    CallMath {
        id: FnId,
        name: String,
        arguments: Vec<Expr>,
        ty: Type,
    },
    /// A call to an ordinary function. `result` names what it returns.
    CallFn {
        id: ExecFnId,
        name: String,
        arguments: Vec<Expr>,
        result: VarId,
        ty: Type,
    },
    /// `then_fact` is `condition == true` and `else_fact` is
    /// `condition == false`, about the comparison the condition performs.
    If {
        condition: Box<Expr>,
        then_fact: HypId,
        else_fact: HypId,
        then_block: Block,
        else_block: Block,
        ty: Type,
        result: VarId,
        joined: Option<Joined>,
    },
    /// One arm per variant, in declaration order, each binding exactly its
    /// variant's payload.
    Match {
        scrutinee: Box<Expr>,
        enum_name: String,
        arms: Vec<MatchArm>,
        ty: Type,
        result: VarId,
        joined: Option<Joined>,
    },
    Block(Block),
    /// `loop { body }`. What the loop carries is the tuple of the bindings
    /// declared outside it that its body assigns (`carried`), and `state`
    /// is the version of each that the body sees at the start of every
    /// pass, in the same order; a proof in the body may mention it. The
    /// loop's value is what `break` supplies, of type `ty`, and is bound
    /// under `result` with `equation` by projection from the carried
    /// tuple's last field. A loop that never breaks produces no value.
    Loop {
        state: Vec<Binder>,
        carried: Carried,
        ty: Type,
        result: VarId,
        equation: HypId,
        body: Block,
    },
    /// `while condition { body }`: a loop whose body tests the condition,
    /// under `then_fact` (`condition == true`, about the comparison
    /// performed) runs `body`, and under `else_fact` leaves the loop. The
    /// condition is part of the loop, so what it assigns is carried too.
    /// Its value is `()` and `break` carries nothing.
    While {
        condition: Box<Expr>,
        then_fact: HypId,
        else_fact: HypId,
        state: Vec<Binder>,
        carried: Carried,
        body: Block,
    },
    /// `for index in lo..hi { body }`, or `lo..=hi` when `inclusive`. The
    /// index is an immutable binding of the bounds' machine type, and the
    /// body has `lo <= index` as `lower` and `index < hi` (`index <= hi`
    /// when inclusive) as `upper`, over the views, afresh on each pass. An
    /// empty range runs no pass. Its value is `()` and `break` carries
    /// nothing.
    For {
        index: Binder,
        lower: HypId,
        upper: HypId,
        lo: Box<Expr>,
        hi: Box<Expr>,
        inclusive: bool,
        state: Vec<Binder>,
        carried: Carried,
        body: Block,
    },
    /// `break`, or `break value` in a `loop`.
    Break(Option<Box<Expr>>),
    Continue,
    /// `return`, or `return value`: the function ends with the value, `()`
    /// when none is written, from any depth of loops and branches. Like
    /// `break` it yields no value, so it stands where any type is expected;
    /// `ty` is that type, and `result` the identity lowering gives the value
    /// it never produces where it is not the end of a block, as for a
    /// panic. The value is checked against the function's result type in
    /// the context of the return.
    Return {
        value: Option<Box<Expr>>,
        ty: Type,
        result: VarId,
    },
    /// Any proof expression: a hole that was filled, a lemma call, a proof
    /// constructor. It prints as `Proved`.
    Proof(Proof),
    /// A proposition value. It prints as `Ghost`.
    Prop(Term),
    /// `match proof {}` used for its value.
    Absurd {
        proof: Proof,
        ty: Type,
    },
    /// `panic!`, `todo!`, or `unreachable!`, with its string argument when
    /// it had one: the call ends in a panic with the form's message, and
    /// nothing follows. It yields no value, so it stands where any type is
    /// expected; `ty` is that type, and `result` the identity lowering gives
    /// the value it never produces where it is not the end of a block.
    /// `unreachable` is evidence of the prelude's `False` at this point,
    /// which a function that promises `no_panic` must have.
    Panic {
        form: PanicForm,
        argument: Option<String>,
        unreachable: Option<Proof>,
        ty: Type,
        result: VarId,
    },
    /// `assert!(condition)` or, with `debug`, `debug_assert!(condition)`,
    /// with `message` what the panic says when the condition is false. Of
    /// type `()`. `then_fact` is `condition == true` and `else_fact` is
    /// `condition == false`, about the comparison the condition performs,
    /// as for an `if`. Afterwards `result` is evidence of `then_fact`'s
    /// claim: the check passed, so the condition is a fact; a
    /// `debug_assert!` is not checked in every build and `result` is then
    /// `()`. `unreachable` is evidence of `False` under `else_fact`, which
    /// `no_panic` requires.
    Assert {
        debug: bool,
        condition: Box<Expr>,
        then_fact: HypId,
        else_fact: HypId,
        message: String,
        unreachable: Option<Proof>,
        result: VarId,
    },
    /// A value of type `Ghost<T>`: the logical value of `expr`, of type
    /// `T`, which has no runtime form. `snapshot!(expr)` builds one, and a
    /// `Ghost<T>` position wraps what stands in it. The expression inside
    /// is elaborated where nothing runs, so it is a term of the logic;
    /// lowering reads through the node, and erasure replaces it by the
    /// marker, keeping whatever inside it would still run.
    Ghost(Box<Expr>),
}

/// Which of the three forms a panic was written as. Each has the message
/// Rust's form of that name prints, with the argument appended after a
/// colon as Rust appends it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanicForm {
    Panic,
    Todo,
    Unreachable,
}

impl PanicForm {
    /// The name before the `!`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Todo => "todo",
            Self::Unreachable => "unreachable",
        }
    }

    /// The message the call ends with: Rust's for the form, or the
    /// argument in its place for `panic!`, and after a colon for the
    /// other two.
    pub fn message(self, argument: Option<&str>) -> String {
        let fixed = match self {
            Self::Panic => return argument.unwrap_or("explicit panic").to_string(),
            Self::Todo => "not yet implemented",
            Self::Unreachable => "internal error: entered unreachable code",
        };
        match argument {
            Some(argument) => format!("{fixed}: {argument}"),
            None => fixed.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MatchArm {
    pub variant_name: String,
    pub payload: Vec<Binder>,
    /// `scrutinee == variant(payload)`.
    pub fact: HypId,
    pub body: Block,
}

impl Expr {
    /// Whether the expression's value is a proof. A `while` or `for` yields
    /// `()` and a comparison yields a `bool`, so neither is one.
    pub fn is_proof(&self) -> bool {
        let proof = |ty: &Type| matches!(ty, Type::Proof(_));
        match self {
            Self::Proof(_) => true,
            Self::Var { ty, .. }
            | Self::Field { ty, .. }
            | Self::CallMath { ty, .. }
            | Self::CallFn { ty, .. }
            | Self::If { ty, .. }
            | Self::Match { ty, .. }
            | Self::Absurd { ty, .. }
            | Self::Panic { ty, .. }
            | Self::Return { ty, .. } => proof(ty),
            Self::Loop { ty, .. } => proof(ty),
            Self::Block(block) => block.tail.as_deref().is_some_and(Self::is_proof),
            _ => false,
        }
    }

    pub fn var(binder: &Binder) -> Self {
        Self::Var {
            id: binder.id,
            name: binder.name.clone(),
            ty: binder.ty.clone(),
        }
    }

    pub fn unit() -> Self {
        Self::Tuple {
            ty: Type::Tuple(Vec::new()),
            fields: Vec::new(),
        }
    }

    /// A `u8` literal.
    pub fn u8(value: u8) -> Self {
        Self::Literal(MachineInt::U8, i128::from(value))
    }
}

impl Binder {
    pub fn new(name: &str, ty: Type) -> Self {
        Self {
            id: VarId::fresh(),
            name: name.to_string(),
            ty,
            ghost: false,
        }
    }

    /// A binder declared `Ghost<T>`, for `ty` the kernel type `T`.
    pub fn ghost(name: &str, ty: Type) -> Self {
        Self {
            ghost: true,
            ..Self::new(name, ty)
        }
    }

    pub fn term(&self) -> Term {
        Term::var(self.id)
    }
}
