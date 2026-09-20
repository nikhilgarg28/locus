//! Surface syntax, including grouping and spans. This is not kernel syntax.

use crate::source::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

/// `Prefix::name`: an enum variant or a proof constructor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub prefix: Name,
    pub name: Name,
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
    Struct {
        name: Name,
        fields: Vec<Parameter>,
    },
    Enum {
        name: Name,
        variants: Vec<Variant>,
    },
    Prop {
        name: Name,
        parameters: Vec<Parameter>,
        variants: Vec<PropVariant>,
    },
    Constant {
        name: Name,
        ty: Type,
        value: Expr,
    },
}

/// `fn` may diverge and runs; `math fn` is pure, total, and usable in logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionMode {
    Runtime,
    Math,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    pub name: Name,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: Name,
    pub fields: Vec<TypeField>,
    pub span: Span,
}

/// A way of proving a declared proposition. `target` is the proposition after
/// `: @`, present when the variant proves the proposition at particular indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropVariant {
    pub name: Name,
    pub fields: Vec<TypeField>,
    pub target: Option<Expr>,
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
    Proof(Box<Expr>),
    Function {
        mode: FunctionMode,
        parameters: Vec<TypeField>,
        result: Box<Type>,
    },
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
    Bool(bool),
    Integer(String),
    Group(Box<Pattern>),
    Tuple(Vec<Pattern>),
    Struct {
        name: Name,
        fields: Vec<PatternField>,
    },
    /// `arguments` is `None` for a variant written without parentheses.
    Variant {
        path: Box<Path>,
        arguments: Option<Vec<Pattern>>,
    },
}

/// `name: pattern`, or the shorthand `name` when `name` is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternField {
    pub name: Option<Name>,
    pub pattern: Pattern,
    pub span: Span,
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
    Path(Box<Path>),
    Integer(String),
    Bool(bool),
    Unit,
    /// `_`: evidence the elaborator is asked to find.
    Hole,
    Group(Box<Expr>),
    Tuple(Vec<Expr>),
    /// `[formula]`: a proposition literal.
    Proposition(Box<Expr>),
    Struct {
        name: Name,
        fields: Vec<ValueField>,
    },
    Block(Block),
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Loop {
        state: Vec<StateParameter>,
        result: Box<Type>,
        body: Block,
    },
    For {
        index: Name,
        lower: Box<Expr>,
        upper: Box<Expr>,
        state: Vec<StateParameter>,
        body: Block,
    },
    Break(Box<Expr>),
    Continue(Vec<Expr>),
    Forall {
        parameters: Vec<Parameter>,
        body: Block,
    },
    Exists {
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
    /// `value.0`
    Index {
        value: Box<Expr>,
        index: String,
        index_span: Span,
    },
    Error,
}

/// `name: value`, or the shorthand `name` when `name` is `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueField {
    pub name: Option<Name>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// `name: Type = initial` in a `loop` or `for` header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateParameter {
    pub name: Name,
    pub ty: Type,
    pub initial: Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
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
