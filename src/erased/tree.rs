//! The erased tree. It mirrors `typed::tree` without its logical content.

use crate::kernel::{EnumId, MachineInt, Prim, StructId, VarId};
use crate::typed::{CompareOp, FnRef};

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
}

#[derive(Clone, Debug)]
pub struct EEnum {
    pub id: EnumId,
    pub name: String,
    pub variants: Vec<EVariant>,
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
    pub result: EType,
    pub body: EBlock,
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
    Call {
        callee: FnRef,
        name: String,
        arguments: Vec<EExpr>,
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
    /// `loop (state = init) { body }`: each state variable with its initial
    /// value.
    Loop {
        state: Vec<(VarId, String, EType, EExpr)>,
        result: EType,
        body: EBlock,
    },
    For {
        index: (VarId, String),
        lo: Box<EExpr>,
        hi: Box<EExpr>,
        state: Vec<(VarId, String, EType, EExpr)>,
        body: EBlock,
    },
    Break(Box<EExpr>),
    Continue(Vec<EExpr>),
    /// `return value`: the call in progress ends with this value, from any
    /// depth of loops and matches. Like `break` it yields no value.
    Return(Box<EExpr>),
    /// A point the program was shown never to reach.
    Trap,
    /// `panic!("message")`: the call in progress ends in a panic with this
    /// message. It has every type, since it yields no value.
    Panic {
        message: String,
    },
}

#[derive(Clone, Debug)]
pub struct EArm {
    pub variant_name: String,
    pub payload: Vec<(VarId, String)>,
    pub body: EBlock,
}
