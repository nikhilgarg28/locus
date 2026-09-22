//! The erased tree. It mirrors `typed::tree` without its logical content.

use crate::kernel::{EnumId, MachineInt, Op, Prim, StructId, VarId};
use crate::typed::{CompareOp, Derive, FnRef, PanicForm, Passing};

/// A simple type: no propositions, no dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EType {
    Bool,
    /// A machine integer type.
    Int(MachineInt),
    /// The erasure of a proof.
    Proved,
    /// The erasure of any other ghost value.
    Ghost,
    Tuple(Vec<EType>),
    Struct(StructId),
    Enum(EnumId),
    Fn(Vec<EType>, Box<EType>),
}

impl EType {
    pub fn unit() -> Self {
        Self::Tuple(Vec::new())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub structs: Vec<EStruct>,
    pub enums: Vec<EEnum>,
    pub fns: Vec<EFn>,
}

#[derive(Clone, Debug)]
pub struct EStruct {
    pub id: StructId,
    pub name: String,
    pub fields: Vec<(String, EType)>,
    /// Printed as `#[derive(...)]`, in this order.
    pub derives: Vec<Derive>,
}

#[derive(Clone, Debug)]
pub struct EEnum {
    pub id: EnumId,
    pub name: String,
    pub variants: Vec<EVariant>,
    /// Printed as `#[derive(...)]`, in this order.
    pub derives: Vec<Derive>,
}

#[derive(Clone, Debug)]
pub struct EVariant {
    pub name: String,
    pub payload: Vec<EType>,
    /// The field names when the variant is written with braces, in the
    /// order of `payload`; `None` for a tuple or unit variant.
    pub fields: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct EFn {
    pub reference: FnRef,
    pub name: String,
    /// Declared with `const`: printed as a Rust `const` item, whose body is
    /// the tail of `body`, and named without a call.
    pub constant: bool,
    pub params: Vec<(VarId, String, EType)>,
    /// How each parameter is passed, in the order of `params`; a shorter
    /// list means the rest are by value. A `&T` or `&mut T` parameter has
    /// the type lent as its `EType`: the printer writes the reference, and
    /// the interpreter, which passes values, writes a `&mut` one back to
    /// the place its caller lent. `MutValue` is kept only when the body
    /// assigns the parameter, since Rust warns of a `mut` it never needs.
    pub passing: Vec<Passing>,
    pub result: EType,
    pub body: EBlock,
}

impl EFn {
    /// How the parameter at `index` is passed.
    pub fn passing_of(&self, index: usize) -> Passing {
        self.passing.get(index).copied().unwrap_or_default()
    }

    /// The positions of the parameters passed by `&mut`, in order.
    pub fn lent(&self) -> Vec<usize> {
        (0..self.params.len())
            .filter(|&index| self.passing_of(index) == Passing::RefMut)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct EBlock {
    pub stmts: Vec<EStmt>,
    pub tail: Option<Box<EExpr>>,
}

#[derive(Clone, Debug)]
pub enum EStmt {
    Let {
        pattern: EPattern,
        value: EExpr,
    },
    /// `place = value;`: the binding, or the field of it the path names, is
    /// replaced in place. The versions of the typed tree are gone: every
    /// mention of the binding is the binding.
    Assign {
        place: EPlace,
        value: EExpr,
    },
    Expr(EExpr),
}

/// The left side of an assignment: a binding and a path of fields into it.
#[derive(Clone, Debug)]
pub struct EPlace {
    pub id: VarId,
    pub name: String,
    /// Each field by position, with its name when it has one.
    pub path: Vec<(usize, Option<String>)>,
}

#[derive(Clone, Debug)]
pub enum EPattern {
    /// A name with its type. The type is what the name has when the value
    /// bound never yields, where there is no value to take a type from, and
    /// it is what the printer writes there so that Rust need not infer it.
    /// `mutable` is `let mut`, and is set only when the function assigns
    /// to the name, since Rust warns of a `mut` that is never needed.
    Bind {
        id: VarId,
        name: String,
        ty: EType,
        mutable: bool,
    },
    Wildcard,
    Tuple(Vec<EPattern>),
}

#[derive(Clone, Debug)]
pub enum EExpr {
    Var {
        id: VarId,
        name: String,
    },
    Bool(bool),
    /// A literal of a machine integer type, within its range.
    Literal(MachineInt, i128),
    Proved,
    Ghost,
    Tuple(Vec<EExpr>),
    Struct {
        id: StructId,
        name: String,
        fields: Vec<(String, EExpr)>,
    },
    Variant {
        id: EnumId,
        enum_name: String,
        index: usize,
        variant_name: String,
        payload: Vec<EExpr>,
    },
    Field {
        target: Box<EExpr>,
        index: usize,
        name: Option<String>,
    },
    Method {
        prim: Prim,
        receiver: Box<EExpr>,
        arguments: Vec<EExpr>,
    },
    /// A comparison of two values of one type: a machine type, or `bool`
    /// for `==` and `!=`.
    Compare {
        op: CompareOp,
        left: Box<EExpr>,
        right: Box<EExpr>,
    },
    /// `expr as to`, between machine types; it wraps.
    Cast {
        expr: Box<EExpr>,
        to: MachineInt,
    },
    /// `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, or `-a` at a machine
    /// type: a row of the table that may panic, printed as the operator it
    /// is, so that Rust panics exactly where the interpreters do.
    Operate {
        op: Op,
        ty: MachineInt,
        operands: Vec<EExpr>,
    },
    Call {
        callee: FnRef,
        name: String,
        arguments: Vec<EExpr>,
    },
    /// `&place` or `&mut place` as the argument of a call: printed as
    /// written, with a `*` where the root is itself a reference parameter.
    /// The interpreter passes the place's value, and after the call writes
    /// the callee's final value of a `&mut` one back to the place, also
    /// when the callee panics.
    Lend {
        mutable: bool,
        place: EPlace,
    },
    If {
        condition: Box<EExpr>,
        then_block: EBlock,
        else_block: EBlock,
    },
    Match {
        scrutinee: Box<EExpr>,
        enum_name: String,
        arms: Vec<EArm>,
    },
    Block(EBlock),
    /// `loop { body }`. What it carries is assigned in place; `result` is
    /// the type of the value a `break` supplies, which is the loop's.
    Loop {
        result: EType,
        body: EBlock,
    },
    /// `while condition { body }`.
    While {
        condition: Box<EExpr>,
        body: EBlock,
    },
    /// `for index in lo..hi { body }`, or `lo..=hi` when `inclusive`. The
    /// index has the bounds' machine type.
    For {
        index: (VarId, String, MachineInt),
        lo: Box<EExpr>,
        hi: Box<EExpr>,
        inclusive: bool,
        body: EBlock,
    },
    /// `break`, or `break value` in a `loop`.
    Break(Option<Box<EExpr>>),
    Continue,
    /// `return value`: the call in progress ends with this value, from any
    /// depth of loops and matches. Like `break` it yields no value.
    Return(Box<EExpr>),
    /// A point the program was shown never to reach.
    Trap,
    /// `panic!`, `todo!`, or `unreachable!`, with its string argument when
    /// it had one: the call in progress ends in a panic with the form's
    /// message, `PanicForm::message`. It has every type, since it yields
    /// no value.
    Panic {
        form: PanicForm,
        argument: Option<String>,
    },
    /// `assert!(condition)` or, with `debug`, `debug_assert!(condition)`:
    /// the call in progress ends in a panic with `message` when the
    /// condition is false. Of type `()`.
    Assert {
        debug: bool,
        condition: Box<EExpr>,
        message: String,
    },
}

#[derive(Clone, Debug)]
pub struct EArm {
    pub variant_name: String,
    pub payload: Vec<(VarId, String)>,
    pub body: EBlock,
}
