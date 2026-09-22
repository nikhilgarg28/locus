//! The check IR. Binders carry identities chosen by whoever built the
//! program, because the program's proofs refer to them; the checker binds
//! the same identities in the kernel context.

use crate::kernel::{HypId, MachineInt, Op, Proof, Term, Type, VarId};

/// Identity of a declared ordinary function; see `Program`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExecFnId(pub(super) usize);

/// What a function promises about every call of it. The checker enforces
/// each promise a function makes; see `Program::declare`. The default is to
/// promise nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Promises {
    /// Every call returns or panics.
    pub terminates: bool,
    /// No call panics.
    pub no_panic: bool,
    /// No call allocates.
    pub no_alloc: bool,
    /// No call performs I/O.
    pub no_io: bool,
}

impl Promises {
    pub fn makes(self, promise: Promise) -> bool {
        match promise {
            Promise::Terminates => self.terminates,
            Promise::NoPanic => self.no_panic,
            Promise::NoAlloc => self.no_alloc,
            Promise::NoIo => self.no_io,
        }
    }
}

/// One of the four promises, for naming the one that was broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Promise {
    Terminates,
    NoPanic,
    NoAlloc,
    NoIo,
}

impl Promise {
    pub const ALL: [Self; 4] = [Self::Terminates, Self::NoPanic, Self::NoAlloc, Self::NoIo];

    pub fn name(self) -> &'static str {
        match self {
            Self::Terminates => "terminates",
            Self::NoPanic => "no_panic",
            Self::NoAlloc => "no_alloc",
            Self::NoIo => "no_io",
        }
    }
}

/// An ordinary `fn`. Unless it promises otherwise it may diverge or panic,
/// and it is checked for partial correctness.
#[derive(Clone, Debug)]
pub struct ExecFn {
    pub promises: Promises,
    /// A kernel function type: a parameter telescope and a result type that
    /// may mention the parameters.
    pub signature: Type,
    /// The identities the body uses for the parameters, in order.
    pub params: Vec<VarId>,
    pub body: Block,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Tail,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    /// `let var: ty = value`, for a pure, total value. The checker declares
    /// `var` and assumes `var == value` as `equation`; a proof has no
    /// equation. When an annotation is given, the value must have that type.
    Let {
        var: VarId,
        equation: HypId,
        ty: Option<Type>,
        value: Term,
    },
    /// `let hyp: @claim = proof`. The proof is checked where it stands, and
    /// the claim is available afterwards.
    Have {
        hyp: HypId,
        claim: Term,
        proof: Proof,
    },
    /// `let var = callee(arguments)`, a call to an ordinary function. It may
    /// not return. If it does, `var` has the callee's result type at these
    /// arguments, and nothing else is known about it.
    Call {
        var: VarId,
        callee: ExecFnId,
        arguments: Vec<Term>,
    },
    /// `let var: ty = match scrutinee { arms }`. An arm either produces a
    /// value of type `ty` or transfers control. When every arm transfers
    /// control, nothing after the match is reached.
    Match {
        var: VarId,
        ty: Type,
        scrutinee: Term,
        arms: Vec<Arm>,
    },
    /// `let var = loop (vars: state = init) -> result { body }`. `state` is
    /// a tuple telescope. The body must not reach its end: every path ends
    /// in `break`, `continue`, `return`, or a panic.
    Loop {
        var: VarId,
        state: Type,
        vars: Vec<VarId>,
        init: Vec<Term>,
        result: Type,
        body: Block,
    },
    /// A bounded `for`; see `ForStmt`.
    For(Box<ForStmt>),
    /// `let var = op[ty](arguments)`, a primitive operation of the table
    /// in `src/kernel/ops.rs` that may panic: `+`, `-`, `*`, `/`, `%`, or
    /// unary minus at a machine type. See `OperateStmt`.
    Operate(Box<OperateStmt>),
}

/// `let var = op[ty](arguments)`: an operator applied at runtime, where
/// Rust may panic. The checker gives `var` the equation `var ==[ty]
/// op[ty](arguments)` as `equation`, which by `op_model` is the wrapped
/// result, the one meaning that holds in every build.
///
/// `fits` is the evidence that the operation does not panic: one proof per
/// premise of `Row::fits`, in its order. For `+`, `-`, `*`, and unary minus
/// the premises are `min(ty) <= e` and `e <= max(ty)` for the exact result
/// `e` of the views; for `/` and `%` they are `view(b) != 0` and, at a
/// signed type, that the pair is not `min / -1`. A function that promises
/// `no_panic` must give it at every row that can panic.
///
/// `learned` names the hypotheses that hold after the statement: for `+`,
/// `-`, `*`, and unary minus with `fits`, one hypothesis, the exact result
/// `view[ty](var) ==[Int] e`, derived from `op_exact` and the proofs; for
/// `/` and `%`, one per premise, whether or not `fits` is given, because
/// these panic in every build and execution continues only past a divisor
/// that was not zero. Nothing else is learned: an overflow that only some
/// builds check teaches nothing, and the wrapping methods are terms, never
/// statements.
#[derive(Clone, Debug)]
pub struct OperateStmt {
    pub var: VarId,
    pub equation: HypId,
    pub op: Op,
    pub ty: MachineInt,
    pub arguments: Vec<Term>,
    pub fits: Option<Vec<Proof>>,
    pub learned: Vec<HypId>,
}

/// `let var = for index in lo..hi (vars: state = init) { body }`.
///
/// `state` is a function type from the index to the state's tuple type,
/// `math fn(T) -> (A_0, ..., A_n)` for the machine type `T` of the index and
/// the bounds, which is how a state type mentions the index: an invariant
/// can say what holds after `index` steps. `ordered` proves
/// `int_le(view[T](lo), view[T](hi))`. The body sees the index, abstract
/// state, and the facts `lo <= index` (`lower`) and `index < hi` (`upper`),
/// both over the views, and must end every path in `continue` with the
/// state for `wrapping_add[T](index, 1)`, in `return`, or in a panic. There
/// is no `break`. `var` is the state at `hi`.
#[derive(Clone, Debug)]
pub struct ForStmt {
    pub var: VarId,
    pub index: VarId,
    pub lower: HypId,
    pub upper: HypId,
    pub lo: Term,
    pub hi: Term,
    pub ordered: Proof,
    pub state: Type,
    pub vars: Vec<VarId>,
    pub init: Vec<Term>,
    pub body: Block,
}

/// One arm of a match: the payload's identities, the identity of the fact
/// `scrutinee == variant(payload)`, and the arm's block. Arms are in variant
/// order; for `bool` that is `false`, then `true`.
#[derive(Clone, Debug)]
pub struct Arm {
    pub payload: Vec<VarId>,
    pub fact: HypId,
    pub body: Block,
}

#[derive(Clone, Debug)]
pub enum Tail {
    /// The block's result.
    Value(Term),
    /// Leaves the nearest enclosing loop with its result. Not available when
    /// the nearest enclosing iteration is a `for`.
    Break(Term),
    /// Starts the next iteration of the nearest enclosing loop or `for` with
    /// this state.
    Continue(Vec<Term>),
    /// A match in tail position: each arm is a block of the same kind.
    Match { scrutinee: Term, arms: Vec<Arm> },
    /// Leaves the function with its result, from any depth of loops and
    /// matches. The value is checked against the function's result type in
    /// the context of this point, exactly as the value of the body is. Like
    /// `break`, it means the block produces no value.
    Return(Term),
    /// The call ends in a panic with this message, and nothing follows. It
    /// demands nothing: a call that panics does not return, and partial
    /// correctness speaks only of returning. In a function that promises
    /// `no_panic`, `unreachable` is required: a proof of the prelude's
    /// `False` in the context of this point, which shows that no run gets
    /// here. A proof that is given is checked either way.
    Panic {
        message: String,
        unreachable: Option<Proof>,
    },
}
