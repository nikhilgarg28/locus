//! The erased tree. It mirrors `typed::tree` without its logical content.

use crate::kernel::{EnumId, Prim, StructId, VarId};
use crate::typed::{CompareOp, FnRef};

/// A simple type: no propositions, no dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EType {
    Bool,
    U8,
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
}

#[derive(Clone, Debug)]
pub struct EFn {
    pub reference: FnRef,
    pub name: String,
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
    Let { pattern: EPattern, value: EExpr },
    Expr(EExpr),
}

#[derive(Clone, Debug)]
pub enum EPattern {
    Bind { id: VarId, name: String },
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
    U8(u8),
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
    Compare {
        op: CompareOp,
        left: Box<EExpr>,
        right: Box<EExpr>,
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
    /// A point the program was shown never to reach.
    Trap,
}

#[derive(Clone, Debug)]
pub struct EArm {
    pub variant_name: String,
    pub payload: Vec<(VarId, String)>,
    pub body: EBlock,
}
