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
    /// Leaves the nearest enclosing loop with its result.
    Break(Term),
    /// Starts the nearest enclosing loop's next iteration with this state.
    Continue(Vec<Term>),
    /// A match in tail position: each arm is a block of the same kind.
    Match { scrutinee: Term, arms: Vec<Arm> },
}
