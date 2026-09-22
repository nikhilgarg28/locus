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
//! and arm, the equation of each `let`, and the result of each call, `if`,
//! `match`, and loop, which needs a name only if lowering has to name it.

use crate::exec::ExecFnId;
use crate::kernel::{
    EnumId, FnId, HypId, Integer, MachineInt, Prim, Proof, StructId, Term, Type, VarId,
};

/// A binding occurrence: an identity, the spelling to print, and its type.
#[derive(Clone, Debug)]
pub struct Binder {
    pub id: VarId,
    pub name: String,
    pub ty: Type,
}

/// `struct Name { field: Type, ... }`. A field's type may mention the
/// fields before it, by their binders' identities.
#[derive(Clone, Debug)]
pub struct StructItem {
    pub name: String,
    pub fields: Vec<Binder>,
}

#[derive(Clone, Debug)]
pub struct EnumItem {
    pub name: String,
    pub variants: Vec<VariantItem>,
}

#[derive(Clone, Debug)]
pub struct VariantItem {
    pub name: String,
    pub payload: Vec<Binder>,
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
    Let { pattern: Pattern, value: Expr },
    Expr(Expr),
}

#[derive(Clone, Debug)]
pub enum Pattern {
    /// A name, with the identity of the equation `name == value`. A proof
    /// has no equation and the identity is unused.
    Bind {
        binder: Binder,
        equation: HypId,
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
    },
    /// One arm per variant, in declaration order, each binding exactly its
    /// variant's payload.
    Match {
        scrutinee: Box<Expr>,
        enum_name: String,
        arms: Vec<MatchArm>,
        ty: Type,
        result: VarId,
    },
    Block(Block),
    Loop {
        state: Vec<(Binder, Expr)>,
        result_ty: Type,
        body: Block,
        result: VarId,
    },
    For {
        index: Binder,
        lower: HypId,
        upper: HypId,
        lo: Box<Expr>,
        hi: Box<Expr>,
        ordered: Proof,
        state: Vec<(Binder, Expr)>,
        body: Block,
        result: VarId,
    },
    Break(Box<Expr>),
    Continue(Vec<Expr>),
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
    /// Whether the expression's value is a proof. A `for` yields its state
    /// tuple and a comparison yields a `bool`, so neither is one.
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
            | Self::Absurd { ty, .. } => proof(ty),
            Self::Loop { result_ty, .. } => proof(result_ty),
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
        }
    }

    pub fn term(&self) -> Term {
        Term::var(self.id)
    }
}
