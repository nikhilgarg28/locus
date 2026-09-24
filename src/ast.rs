//! Surface syntax, including grouping and spans. This is not kernel syntax.

use crate::kernel::Natural;
use crate::source::Span;

/// The type an integer literal names after its digits, as in `7u8`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntegerSuffix {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
}

impl IntegerSuffix {
    pub const ALL: [Self; 12] = [
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::Usize,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
        Self::Isize,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::Usize => "usize",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
            Self::Isize => "isize",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|suffix| suffix.name() == name)
    }
}

/// An integer literal: its value, whatever its size and base, and its suffix.
/// Whether the value fits a type is the elaborator's question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegerLiteral {
    pub value: Natural,
    pub suffix: Option<IntegerSuffix>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

/// `a::b::c`: segments joined by `::`, at least one. A path that begins
/// with `crate`, `super`, `self`, or `Self` has that keyword as its first
/// segment. Where a name stands on its own it is a `Name`, not a path of
/// one segment; a one-segment path occurs only where a path is required,
/// as in a `derive` list or a struct literal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<Name>,
    pub span: Span,
}

impl Path {
    /// The last segment: the variant of `Enum::Variant`, the constant of
    /// `u32::MAX`.
    pub fn last(&self) -> &Name {
        self.segments.last().expect("a path has a segment")
    }

    /// The one segment of a path that has just one.
    pub fn single(&self) -> Option<&Name> {
        match self.segments.as_slice() {
            [name] => Some(name),
            _ => None,
        }
    }

    /// The two segments of `Prefix::name`, when the path has just those.
    pub fn pair(&self) -> Option<(&Name, &Name)> {
        match self.segments.as_slice() {
            [prefix, name] => Some((prefix, name)),
            _ => None,
        }
    }

    /// The path as written, without spaces.
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<Vec<_>>()
            .join("::")
    }
}

/// A `///` or `//!` line, or a `/** */` or `/*! */` block: the text after
/// the marker, as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocComment {
    pub text: String,
    pub span: Span,
}

/// `#[name]` or `#[name(arguments)]` before an item, or `#![...]` at the
/// top of a file. The set is closed: the parser knows every attribute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub kind: AttributeKind,
    /// Whole, from `#` to `]`.
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeKind {
    /// Kept with the item; only the `trusted "reason" fn ... = Rust::item;`
    /// declaration syntax constructs this, never a detachable attribute.
    Trusted {
        reason: String,
        implementation: Path,
    },
    /// `#[terminates]`, or `#[terminates(decreases = expression)]`.
    Terminates {
        decreases: Option<Expr>,
    },
    NoPanic,
    NoAlloc,
    NoIo,
    /// `#[derive(Clone, Copy)]`: traits by path.
    Derive(Vec<Path>),
}

impl AttributeKind {
    /// The names of the closed set, in the order they are listed in.
    pub const NAMES: [&'static str; 5] = ["terminates", "no_panic", "no_alloc", "no_io", "derive"];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Terminates { .. } => "terminates",
            Self::NoPanic => "no_panic",
            Self::NoAlloc => "no_alloc",
            Self::NoIo => "no_io",
            Self::Derive(_) => "derive",
            Self::Trusted { .. } => "trusted",
        }
    }

    /// A promise about an effect, as opposed to `derive`.
    pub fn is_promise(&self) -> bool {
        !matches!(self, Self::Derive(_) | Self::Trusted { .. })
    }
}

/// `pub` and its restricted forms. An item or field without one is private.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Visibility {
    pub scope: VisibilityScope,
    /// From `pub` to the end of its parenthesized scope, if any.
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VisibilityScope {
    /// Plain `pub`.
    Public,
    /// `pub(crate)`
    Crate,
    /// `pub(super)`
    Super,
    /// `pub(self)`, which means private, as it does in Rust.
    SelfModule,
    /// `pub(in path)`
    In(Path),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    /// `//!` comments at the top of the file.
    pub doc: Vec<DocComment>,
    /// `#![...]` attributes at the top of the file: promises for every
    /// function in it.
    pub attributes: Vec<Attribute>,
    pub declarations: Vec<Declaration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub doc: Vec<DocComment>,
    pub attributes: Vec<Attribute>,
    pub visibility: Option<Visibility>,
    pub kind: DeclarationKind,
    /// From the first doc comment, attribute, or `pub` to the end of the item.
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    /// A source module; `None` is loaded from its declared file by the project loader.
    Module {
        name: Name,
        body: Option<Program>,
    },
    /// Explicit imports; grouped syntax is expanded into these leaves by the parser.
    Use {
        imports: Vec<Import>,
    },
    Function {
        /// `logic fn` is an erased logical definition.
        logical: bool,
        generics: Vec<GenericParameter>,
        name: Name,
        /// The receiver of a method in an `impl` block.
        self_param: Option<SelfParam>,
        parameters: Vec<Parameter>,
        result: Type,
        body: Block,
    },
    Struct {
        generics: Vec<GenericParameter>,
        name: Name,
        fields: Vec<Field>,
    },
    Enum {
        generics: Vec<GenericParameter>,
        name: Name,
        variants: Vec<Variant>,
    },
    Prop {
        generics: Vec<GenericParameter>,
        name: Name,
        parameters: Vec<Parameter>,
        variants: Vec<PropVariant>,
    },
    Constant {
        name: Name,
        ty: Type,
        value: Expr,
    },
    /// `impl Name { ... }`: methods, associated functions, and constants,
    /// each with its own documentation, attributes, and visibility.
    Impl {
        /// The compiler-known Model bridge; ordinary inherent impls have none.
        model: Option<ModelImpl>,
        /// The type, by name or by path.
        target: Path,
        methods: Vec<Declaration>,
    },
}

/// `impl Model for Source { type Logic = Target; ... }`: a checked canonical observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelImpl {
    pub source: Type,
    pub target: Type,
    pub span: Span,
}

/// A type parameter, optionally constrained by named bounds, such as
/// `T: Logical`. Bounds are checked by the elaborator, not the parser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericParameter {
    /// Lifetime parameters are checked scopes, never type specializations.
    pub lifetime: bool,
    pub name: Name,
    pub bounds: Vec<Path>,
    pub span: Span,
}

/// The receiver of a method: `self`, `mut self`, `&self`, or `&mut self`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfParam {
    pub kind: SelfKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfKind {
    /// `self`
    Value,
    /// `mut self`
    MutValue,
    /// `&self`
    Ref,
    /// `&mut self`
    RefMut,
}

impl SelfKind {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Value => "self",
            Self::MutValue => "mut self",
            Self::Ref => "&self",
            Self::RefMut => "&mut self",
        }
    }
}

/// `name: Type`, or `mut name: Type` for a parameter the body may assign
/// (polish: it parses and is stored, and the elaborator does not read it yet).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    pub mutable: bool,
    pub name: Name,
    pub ty: Type,
    /// From the name, or the `mut` before it, to the end of the type.
    pub span: Span,
}

/// A field of a struct: `pub name: Type`, private without the `pub`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub doc: Vec<DocComment>,
    pub visibility: Option<Visibility>,
    pub name: Name,
    pub ty: Type,
    /// From the name to the end of the type.
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub doc: Vec<DocComment>,
    pub name: Name,
    pub shape: VariantShape,
    /// Every field has a name when the shape is `Struct`.
    pub fields: Vec<TypeField>,
    pub span: Span,
}

/// How a variant's fields are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantShape {
    /// `Name`
    Unit,
    /// `Name(fields)`
    Tuple,
    /// `Name { name: Type, ... }`
    Struct,
}

/// A way of proving a declared proposition. `target` is the proposition after
/// `: @`, present when the variant proves the proposition at particular indices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropVariant {
    pub doc: Vec<DocComment>,
    pub name: Name,
    pub shape: VariantShape,
    /// Every field has a name when the shape is `Struct`.
    pub fields: Vec<TypeField>,
    pub target: Option<Expr>,
    /// The computed proposition after `=>`; absent only in legacy syntax.
    pub body: Option<Block>,
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
    /// A nominal lifetime argument, written with its leading apostrophe.
    Lifetime(Name),
    /// A type named by a path of two or more segments, or by a name with
    /// type arguments: `a::B`, `Option<T>`, `crate::a::Map<K, V>`. The path
    /// is boxed so that a `Type` stays the size it was: the parser keeps
    /// types in the frames of its recursion.
    Path {
        path: Box<Path>,
        arguments: Vec<Type>,
    },
    Unit,
    /// Borrowed slice element type, `[T]`.
    Slice(Box<Type>),
    /// Fixed-size array type, `[T; n]`.
    Array {
        element: Box<Type>,
        length: Box<Expr>,
    },
    Group(Box<Type>),
    Tuple(Vec<TypeField>),
    Proof(Box<Expr>),
    /// `fn(T, U) -> R`, or with named parameters `fn(x: T) -> @(x == x)`:
    /// the type of a function, which a proposition applies.
    Function {
        parameters: Vec<TypeField>,
        result: Box<Type>,
    },
    /// `logic Fn(x: T) -> R`: an erased logical callable.
    LogicalFunction {
        parameters: Vec<TypeField>,
        result: Box<Type>,
    },
    /// `&T` or `&mut T`.
    Ref {
        lifetime: Option<Name>,
        mutable: bool,
        inner: Box<Type>,
    },
    /// `!`, the type of an expression that never produces a value.
    Never,
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
    /// A binding, `name` or `mut name`.
    Name {
        name: Name,
        mutable: bool,
    },
    /// `name @ pattern`: the whole value and a pattern of its parts.
    Binding {
        name: Name,
        mutable: bool,
        pattern: Box<Pattern>,
        at_span: Span,
    },
    /// `P::Arm(witnesses) @ evidence`: proof of a named proposition arm.
    Evidence {
        constructor: Box<Pattern>,
        evidence: Box<Pattern>,
        at_span: Span,
    },
    Wildcard,
    Unit,
    Bool(bool),
    Integer(IntegerLiteral),
    Group(Box<Pattern>),
    Tuple(Vec<Pattern>),
    /// `S { fields }`, or `E::V { fields }` for a variant with named
    /// fields; `rest` is the span of a `..` after the fields.
    Struct {
        path: Box<Path>,
        fields: Vec<PatternField>,
        rest: Option<Span>,
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
    /// `let pattern = value;`, with `: Type` after the pattern when the
    /// binding is annotated. `mutable` is set for `let mut name`, which is
    /// also recorded on the pattern's name: the one `mut` is read either way.
    Let {
        mutable: bool,
        pattern: Pattern,
        annotation: Option<Type>,
        value: Expr,
    },
    /// `place = value;`, where `place` is a name or a field path such as
    /// `a.b.c` or `a.0.b`; the parser rejects any other left-hand side.
    /// Assignment is a statement, not an expression as in Rust, so it has
    /// no value and cannot stand where a value is needed.
    Assign {
        place: Expr,
        value: Expr,
    },
    /// An expression followed by `;`, or one that ends in a block (`if`,
    /// `match`, `loop`, `for`, `while`, or a block itself), which ends its
    /// statement without a `;` as it does in Rust.
    Expression(Expr),
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    /// Whether the expression ends in a block: a block, `if`, `match`,
    /// `loop`, `for`, or `while`. In statement position such an expression
    /// is a whole statement without a `;`, and what follows begins the next.
    pub fn is_block_like(&self) -> bool {
        matches!(
            self.kind,
            ExprKind::Block(_)
                | ExprKind::Logic(_)
                | ExprKind::If { .. }
                | ExprKind::Match { .. }
                | ExprKind::Loop { .. }
                | ExprKind::For { .. }
                | ExprKind::While { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Name(Name),
    Path(Box<Path>),
    Integer(IntegerLiteral),
    /// A string literal, with its escapes decoded.
    String(String),
    Bool(bool),
    Unit,
    /// `_`: evidence the elaborator is asked to find.
    Hole,
    Group(Box<Expr>),
    Tuple(Vec<Expr>),
    /// Physical array literal, `[a, b, c]`.
    Array(Vec<Expr>),
    /// Physical indexed access, `items[index]`.
    Subscript {
        value: Box<Expr>,
        index: Box<Expr>,
    },
    /// `name!(arguments)`: a built-in form. `prop!` and `prove!` take one
    /// formula; the others take expressions.
    Form {
        form: Form,
        /// The name, without its `!`.
        name_span: Span,
        arguments: Vec<Expr>,
        /// Specialization's physical source hint for model! dependency ordering.
        /// Elaboration independently checks the path and selected model.
        source_hint: Option<Box<Type>>,
    },
    /// `S { fields }`, or `E::V { fields }` for a variant with named fields.
    Struct {
        path: Path,
        fields: Vec<ValueField>,
    },
    Block(Block),
    /// `logic { ... }`: pure, total logical computation.
    Logic(Block),
    /// A named-arm constructor with evidence outside its witness arguments.
    Evidence {
        constructor: Box<Expr>,
        evidence: Box<Expr>,
        at_span: Span,
    },
    If {
        condition: Box<Expr>,
        then_branch: Block,
        else_branch: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// `loop { body }`.
    Loop {
        body: Block,
    },
    /// `while condition { body }`, or `while let pattern = condition { body }`
    /// when `pattern` is present, in which case `condition` is the scrutinee.
    While {
        pattern: Option<Box<Pattern>>,
        condition: Box<Expr>,
        body: Block,
    },
    /// `for pattern in iterable { body }`. The iterable of a bounded loop is
    /// a `Range`; any other expression is an iterator, which comes later.
    For {
        pattern: Box<Pattern>,
        iterable: Box<Expr>,
        body: Block,
    },
    /// `lower..upper` or `lower..=upper`. Only the header of a `for` reads a
    /// range; elsewhere `..` is not an operator yet.
    Range {
        kind: RangeKind,
        lower: Box<Expr>,
        upper: Box<Expr>,
    },
    /// `break`, or `break value`.
    Break(Option<Box<Expr>>),
    /// `continue`, which carries nothing.
    Continue,
    /// `return`, or `return value`.
    Return(Option<Box<Expr>>),
    /// `&value` or `&mut value`.
    Ref {
        mutable: bool,
        expr: Box<Expr>,
    },
    Forall {
        parameters: Vec<Parameter>,
        body: Block,
    },
    Exists {
        parameters: Vec<Parameter>,
        body: Block,
    },
    /// `!value`, which reads as a proposition as well as a `bool`.
    Not(Box<Expr>),
    /// A prefix operator other than `!`.
    Unary {
        operator: UnaryOp,
        operator_span: Span,
        expr: Box<Expr>,
    },
    Binary {
        operator: BinaryOp,
        operator_span: Span,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `expr as Type`
    Cast {
        expr: Box<Expr>,
        /// Filled by specialization for precise Model dependency ordering.
        source_hint: Option<Box<Type>>,
        as_span: Span,
        ty: Type,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    /// An erased logical closure, `|x: T| body`.
    Closure {
        parameters: Vec<Parameter>,
        body: Box<Expr>,
    },
    /// Explicit type arguments, `f::<T>` or `Enum::<T>::Variant`.
    GenericApply {
        callee: Box<Expr>,
        arguments: Vec<Type>,
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

/// Whether a range includes its upper bound: `..` or `..=`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeKind {
    Exclusive,
    Inclusive,
}

impl RangeKind {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Exclusive => "..",
            Self::Inclusive => "..=",
        }
    }
}

/// A built-in form, spelled `name!(...)` as Rust spells a macro call. The
/// list is closed: the parser knows it, and the elaborator gives each form
/// its meaning. Nothing is expanded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Form {
    /// `prop!(formula)`: a proposition.
    Prop,
    /// `prove!(formula)`: a claim stated where it stands, and its evidence.
    Prove,
    Rewrite,
    Unfold,
    Fold,
    Old,
    Snapshot,
    Model,
    Recurse,
    Assert,
    Unreachable,
    Todo,
    Panic,
    DebugAssert,
    Matches,
    Vec,
}

impl Form {
    pub const ALL: [Self; 16] = [
        Self::Prop,
        Self::Prove,
        Self::Rewrite,
        Self::Unfold,
        Self::Fold,
        Self::Old,
        Self::Snapshot,
        Self::Model,
        Self::Recurse,
        Self::Assert,
        Self::Unreachable,
        Self::Todo,
        Self::Panic,
        Self::DebugAssert,
        Self::Matches,
        Self::Vec,
    ];

    /// The name before the `!`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Prop => "prop",
            Self::Prove => "prove",
            Self::Rewrite => "rewrite",
            Self::Unfold => "unfold",
            Self::Fold => "fold",
            Self::Old => "old",
            Self::Snapshot => "snapshot",
            Self::Model => "model",
            Self::Recurse => "recurse",
            Self::Assert => "assert",
            Self::Unreachable => "unreachable",
            Self::Todo => "todo",
            Self::Panic => "panic",
            Self::DebugAssert => "debug_assert",
            Self::Matches => "matches",
            Self::Vec => "vec",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|form| form.name() == name)
    }

    /// Whether the form's one argument is a formula, in which `forall`,
    /// `exists`, and `=>` are recognized.
    pub fn takes_formula(self) -> bool {
        matches!(self, Self::Prop | Self::Prove)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-value`
    Neg,
    /// `*self`, the value behind the reference receiver of a method (O4).
    /// The parser reads `*` before `self` alone; `*` before anything else
    /// is still a construct of Rust that Locus does not have.
    Deref,
}

impl UnaryOp {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Neg => "-",
            Self::Deref => "*",
        }
    }
}

/// The binary operators, in Rust's order of precedence from tightest to
/// loosest. `as` and the postfix operators bind tighter than all of them,
/// and `=>`, Locus's implication, is below `||`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Shl,
    Shr,
    BitAnd,
    BitXor,
    BitOr,
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
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Mul => "*",
            Self::Div => "/",
            Self::Rem => "%",
            Self::Add => "+",
            Self::Sub => "-",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::BitAnd => "&",
            Self::BitXor => "^",
            Self::BitOr => "|",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::And => "&&",
            Self::Or => "||",
            Self::Implies => "=>",
        }
    }

    /// `+ - * / %`, the arithmetic of the integer types.
    pub fn is_arithmetic(self) -> bool {
        matches!(
            self,
            Self::Add | Self::Sub | Self::Mul | Self::Div | Self::Rem
        )
    }

    /// The shifts and the bitwise operators.
    pub fn is_bitwise(self) -> bool {
        matches!(
            self,
            Self::Shl | Self::Shr | Self::BitAnd | Self::BitXor | Self::BitOr
        )
    }

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

/// A named import or re-export. Globs are deliberately rejected in this tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub path: Path,
    pub alias: Option<Name>,
}
