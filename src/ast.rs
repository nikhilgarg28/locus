//! Surface syntax, including grouping and spans. This is not kernel syntax.

use crate::source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub kind: DeclarationKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    Function {
        mode: FunctionMode,
        name: Name,
        parameters: Vec<Parameter>,
        result: Type,
        body: Block,
    },
    Constant {
        name: Name,
        ty: Type,
        value: Expr,
    },
}

/// Declared execution phase; semantic phase checking follows name resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionMode {
    Runtime,
    Logical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    pub name: Name,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Named(Name),
    Unit,
    Group(Box<Type>),
    Tuple(Vec<TypeField>),
    Array {
        element: Box<Type>,
        length: Box<Expr>,
    },
    Slice(Box<Type>),
    Proof(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeField {
    pub name: Option<Name>,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Name(Name),
    Wildcard,
    Unit,
    Group(Box<Pattern>),
    Tuple(Vec<Pattern>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatementKind {
    Let {
        pattern: Pattern,
        annotation: Option<Type>,
        value: Expr,
    },
    Expression(Expr),
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Name(Name),
    Integer(String),
    Bool(bool),
    Unit,
    Group(Box<Expr>),
    Tuple(Vec<Expr>),
    /// `[e]`: elaboration chooses a proposition or singleton array from context.
    Bracket(Box<Expr>),
    /// Empty or comma-marked array syntax; never a proposition literal.
    Array(Vec<Expr>),
    RepeatArray {
        value: Box<Expr>,
        count: Box<Expr>,
    },
    Block(Block),
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Box<Expr>,
    },
    Forall {
        parameters: Vec<Parameter>,
        body: Block,
    },
    Not(Box<Expr>),
    Binary {
        operator: BinaryOp,
        operator_span: Span,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Member {
        value: Box<Expr>,
        name: Name,
    },
    Proof(ProofRequest),
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Implies,
}

impl BinaryOp {
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Equal
                | Self::NotEqual
                | Self::Less
                | Self::LessEqual
                | Self::Greater
                | Self::GreaterEqual
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProofRequest {
    Inferred,
    Block { commands: Vec<ProofCommand> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofCommand {
    pub name: Name,
    pub arguments: Vec<Expr>,
    pub span: Span,
}
