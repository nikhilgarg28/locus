//! The check IR. Binders carry identities chosen by whoever built the
//! program, because the program's proofs refer to them; the checker binds
//! the same identities in the kernel context.

use crate::kernel::{HypId, Proof, Term, Type, VarId};

/// Identity of a declared ordinary function; see `Program`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExecFnId(pub(super) usize);

/// An ordinary `fn`: it may diverge, and it is checked for partial
/// correctness.
#[derive(Clone, Debug)]
pub struct ExecFn {
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
    /// `let var = value`, for a pure, total value. The checker declares
    /// `var` and assumes `var == value` as `equation`.
    Let {
        var: VarId,
        equation: HypId,
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
    /// value of type `ty` or transfers control.
    Match {
        var: VarId,
        ty: Type,
        scrutinee: Term,
        arms: Vec<Arm>,
    },
    /// `let var = loop (vars: state = init) -> result { body }`. `state` is
    /// a tuple telescope. The body must end every path in `break` or
    /// `continue`.
    Loop {
        var: VarId,
        state: Type,
        vars: Vec<VarId>,
        init: Vec<Term>,
        result: Type,
        body: Block,
    },
    /// `let var = for index in lo..hi (vars: state = init) { body }`.
    ///
    /// `state` is a function type from the index to the state's tuple type,
    /// `math fn(u8) -> (A_0, ..., A_n)`, which is how a state type mentions
    /// the index: an invariant can say what holds after `index` steps.
    /// `ordered` proves `lo <= hi`. The body sees the index, abstract state,
    /// and the facts `lo <= index` (`lower`) and `index < hi` (`upper`), and
    /// must end every path in `continue` with the state for `index + 1`.
    /// There is no `break`. `var` is the state at `hi`.
    For {
        var: VarId,
        index: VarId,
        lower: HypId,
        upper: HypId,
        lo: Term,
        hi: Term,
        ordered: Proof,
        state: Type,
        vars: Vec<VarId>,
        init: Vec<Term>,
        body: Block,
    },
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
}
