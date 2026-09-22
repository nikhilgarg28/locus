//! The order in which declarations are elaborated. An item is checked when it
//! is declared, so everything it mentions must already exist. The core has
//! no recursion, so a cycle is an error.
//!
//! Dependencies are read off the syntax: every name an item writes that is
//! also the name of an item in the namespace the position reads, types
//! (structs, enums, propositions) or values (functions, constants). A name
//! applied, `Name(..)`, is the function of that name when there is one and
//! the proposition otherwise, as the elaborator reads it. A local shadows
//! a value of its name where it is in scope, as it does for the elaborator:
//! a parameter, a pattern, a loop's state, or a quantifier's variable named
//! like a function is that local, not a mention of the function.

use std::collections::{HashMap, HashSet};

use crate::ast::*;

/// Indices into `program.declarations`, dependencies first, and the
/// declarations that are part of a cycle.
pub(super) fn dependency_order(program: &Program) -> (Vec<usize>, Vec<usize>) {
    let index_of: HashMap<(Namespace, &str), usize> = program
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| {
            let namespace = declared_namespace(declaration)?;
            declared_name(declaration).map(|name| ((namespace, name.text.as_str()), index))
        })
        .collect();
    let edges: Vec<Vec<usize>> = program
        .declarations
        .iter()
        .map(|declaration| {
            let mut names = HashSet::new();
            Mentions {
                names: &mut names,
                bound: Vec::new(),
            }
            .declaration(declaration);
            let mut edges: Vec<usize> = names
                .iter()
                .filter_map(|(namespace, name)| match namespace {
                    Namespace::Applied => index_of
                        .get(&(Namespace::Value, name.as_str()))
                        .or_else(|| index_of.get(&(Namespace::Type, name.as_str())))
                        .copied(),
                    read => index_of.get(&(*read, name.as_str())).copied(),
                })
                .collect();
            edges.sort_unstable();
            edges.dedup();
            edges
        })
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum State {
        New,
        Open,
        Done,
    }
    fn visit(
        node: usize,
        edges: &[Vec<usize>],
        state: &mut [State],
        order: &mut Vec<usize>,
        cyclic: &mut Vec<usize>,
    ) {
        match state[node] {
            State::Done => return,
            State::Open => {
                if !cyclic.contains(&node) {
                    cyclic.push(node);
                }
                return;
            }
            State::New => {}
        }
        state[node] = State::Open;
        for &next in &edges[node] {
            visit(next, edges, state, order, cyclic);
        }
        state[node] = State::Done;
        order.push(node);
    }
    let mut state = vec![State::New; edges.len()];
    let (mut order, mut cyclic) = (Vec::new(), Vec::new());
    for node in 0..edges.len() {
        visit(node, &edges, &mut state, &mut order, &mut cyclic);
    }
    order.retain(|node| !cyclic.contains(node));
    (order, cyclic)
}

/// Which of Rust's two namespaces a name is read in; `Applied` is a call,
/// `Name(..)`, which is the value when there is one and the type otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Namespace {
    Type,
    Value,
    Applied,
}

/// The namespace an item declares its name in. An `impl` block declares
/// none of its own.
fn declared_namespace(declaration: &Declaration) -> Option<Namespace> {
    match &declaration.kind {
        DeclarationKind::Function { .. } | DeclarationKind::Constant { .. } => {
            Some(Namespace::Value)
        }
        DeclarationKind::Struct { .. }
        | DeclarationKind::Enum { .. }
        | DeclarationKind::Prop { .. } => Some(Namespace::Type),
        DeclarationKind::Impl { .. } => None,
    }
}

/// The name an item declares. An `impl` block declares none of its own.
pub(super) fn declared_name(declaration: &Declaration) -> Option<&Name> {
    match &declaration.kind {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(name),
        DeclarationKind::Impl { .. } => None,
    }
}

struct Mentions<'a> {
    names: &'a mut HashSet<(Namespace, String)>,
    /// The locals in scope, innermost last.
    bound: Vec<String>,
}

impl Mentions<'_> {
    /// A name where a type is read: a type, the prefix of a path, a struct
    /// literal or pattern. No local shadows a type.
    fn type_name(&mut self, name: &Name) {
        self.names.insert((Namespace::Type, name.text.clone()));
    }

    /// A name where a value is read, unless a local of that name is in
    /// scope.
    fn value_name(&mut self, name: &Name) {
        if !self.bound.contains(&name.text) {
            self.names.insert((Namespace::Value, name.text.clone()));
        }
    }

    /// A name applied, which is a function or a proposition, unless a local
    /// of that name is in scope: evidence applied.
    fn name(&mut self, name: &Name) {
        if !self.bound.contains(&name.text) {
            self.names.insert((Namespace::Applied, name.text.clone()));
        }
    }

    /// The names a pattern binds, into scope.
    fn bind(&mut self, pattern: &Pattern) {
        match &pattern.kind {
            PatternKind::Name { name, .. } => self.bound.push(name.text.clone()),
            PatternKind::Wildcard
            | PatternKind::Unit
            | PatternKind::Bool(_)
            | PatternKind::Integer(_) => {}
            PatternKind::Group(inner) => self.bind(inner),
            PatternKind::Tuple(patterns) => patterns.iter().for_each(|inner| self.bind(inner)),
            PatternKind::Struct { fields, .. } => {
                fields.iter().for_each(|field| self.bind(&field.pattern));
            }
            PatternKind::Variant { arguments, .. } => {
                arguments
                    .iter()
                    .flatten()
                    .for_each(|inner| self.bind(inner));
            }
        }
    }

    /// `visit` with `names` in scope, which go out of it after.
    fn scoped(&mut self, names: impl IntoIterator<Item = String>, visit: impl FnOnce(&mut Self)) {
        let depth = self.bound.len();
        self.bound.extend(names);
        visit(self);
        self.bound.truncate(depth);
    }

    fn declaration(&mut self, declaration: &Declaration) {
        match &declaration.kind {
            DeclarationKind::Function {
                parameters,
                result,
                body,
                ..
            } => {
                let names: Vec<String> = parameters
                    .iter()
                    .map(|parameter| parameter.name.text.clone())
                    .collect();
                // A parameter's type may mention the parameters before it.
                self.scoped(names, |this| {
                    parameters
                        .iter()
                        .for_each(|parameter| this.ty(&parameter.ty));
                    this.ty(result);
                    this.block(body);
                });
            }
            DeclarationKind::Struct { fields, .. } => {
                fields.iter().for_each(|field| self.ty(&field.ty));
            }
            DeclarationKind::Enum { variants, .. } => {
                for variant in variants {
                    variant.fields.iter().for_each(|field| self.ty(&field.ty));
                }
            }
            DeclarationKind::Prop {
                parameters,
                variants,
                ..
            } => {
                let names: Vec<String> = parameters
                    .iter()
                    .map(|parameter| parameter.name.text.clone())
                    .collect();
                self.scoped(names, |this| {
                    parameters
                        .iter()
                        .for_each(|parameter| this.ty(&parameter.ty));
                    for variant in variants {
                        let fields: Vec<String> = variant
                            .fields
                            .iter()
                            .filter_map(|field| field.name.as_ref().map(|name| name.text.clone()))
                            .collect();
                        this.scoped(fields, |this| {
                            variant.fields.iter().for_each(|field| this.ty(&field.ty));
                            // `: @Name(arguments)` names the proposition
                            // being declared, which is not a use of it.
                            if let Some(ExprKind::Call { arguments, .. }) =
                                variant.target.as_ref().map(|target| &target.kind)
                            {
                                arguments.iter().for_each(|argument| this.expr(argument));
                            }
                        });
                    }
                });
            }
            DeclarationKind::Constant { ty, value, .. } => {
                self.ty(ty);
                self.expr(value);
            }
            // Not elaborated yet, so it depends on nothing.
            DeclarationKind::Impl { .. } => {}
        }
    }

    fn ty(&mut self, ty: &Type) {
        match &ty.kind {
            TypeKind::Named(name) => self.type_name(name),
            TypeKind::Path { path, arguments } => {
                self.type_name(&path.segments[0]);
                arguments.iter().for_each(|argument| self.ty(argument));
            }
            TypeKind::Unit => {}
            TypeKind::Group(inner) => self.ty(inner),
            // A field's type may mention the fields before it.
            TypeKind::Tuple(fields) => self.fields(fields, |_| {}),
            TypeKind::Proof(proposition) => self.expr(proposition),
            TypeKind::Function {
                parameters, result, ..
            } => self.fields(parameters, |this| this.ty(result)),
            TypeKind::Ref { inner, .. } => self.ty(inner),
            TypeKind::Never => {}
        }
    }

    fn pattern(&mut self, pattern: &Pattern) {
        match &pattern.kind {
            PatternKind::Name { .. }
            | PatternKind::Wildcard
            | PatternKind::Unit
            | PatternKind::Bool(_)
            | PatternKind::Integer(_) => {}
            PatternKind::Group(inner) => self.pattern(inner),
            PatternKind::Tuple(patterns) => patterns.iter().for_each(|inner| self.pattern(inner)),
            PatternKind::Struct { path, fields, .. } => {
                self.type_name(&path.segments[0]);
                fields.iter().for_each(|field| self.pattern(&field.pattern));
            }
            PatternKind::Variant { path, arguments } => {
                self.type_name(&path.segments[0]);
                for argument in arguments.iter().flatten() {
                    self.pattern(argument);
                }
            }
        }
    }

    /// Named fields, each in the scope of the ones before it, and `rest`
    /// in the scope of all of them.
    fn fields(&mut self, fields: &[TypeField], rest: impl FnOnce(&mut Self)) {
        let depth = self.bound.len();
        for field in fields {
            self.ty(&field.ty);
            if let Some(name) = &field.name {
                self.bound.push(name.text.clone());
            }
        }
        rest(self);
        self.bound.truncate(depth);
    }

    /// A block is a scope: what a `let` binds is in scope to its end.
    fn block(&mut self, block: &Block) {
        let depth = self.bound.len();
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Let {
                    pattern,
                    annotation,
                    value,
                    ..
                } => {
                    self.pattern(pattern);
                    annotation.iter().for_each(|ty| self.ty(ty));
                    self.expr(value);
                    self.bind(pattern);
                }
                StatementKind::Assign { place, value } => {
                    self.expr(place);
                    self.expr(value);
                }
                StatementKind::Expression(expr) => self.expr(expr),
                StatementKind::Error => {}
            }
        }
        block.tail.iter().for_each(|tail| self.expr(tail));
        self.bound.truncate(depth);
    }

    /// A loop's state: its initial values are outside the loop, and its
    /// names are in scope in the body, which `body` visits.
    fn state(&mut self, state: &[StateParameter], body: impl FnOnce(&mut Self)) {
        for parameter in state {
            self.ty(&parameter.ty);
            self.expr(&parameter.initial);
        }
        let names: Vec<String> = state
            .iter()
            .map(|parameter| parameter.name.text.clone())
            .collect();
        self.scoped(names, body);
    }

    /// A pattern's names are in scope in `body`.
    fn with_pattern(&mut self, pattern: &Pattern, body: impl FnOnce(&mut Self)) {
        let depth = self.bound.len();
        self.pattern(pattern);
        self.bind(pattern);
        body(self);
        self.bound.truncate(depth);
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Name(name) => self.value_name(name),
            ExprKind::Path(path) => self.type_name(&path.segments[0]),
            ExprKind::Integer(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Unit
            | ExprKind::Hole
            | ExprKind::Error => {}
            ExprKind::Group(inner)
            | ExprKind::Not(inner)
            | ExprKind::Unary { expr: inner, .. }
            | ExprKind::Ref { expr: inner, .. } => self.expr(inner),
            ExprKind::Break(inner) | ExprKind::Return(inner) => {
                inner.iter().for_each(|inner| self.expr(inner));
            }
            ExprKind::Cast {
                expr: inner, ty, ..
            } => {
                self.expr(inner);
                self.ty(ty);
            }
            ExprKind::Tuple(items) => items.iter().for_each(|item| self.expr(item)),
            ExprKind::Continue(items) => {
                items.iter().flatten().for_each(|item| self.expr(item));
            }
            ExprKind::Range { lower, upper, .. } => {
                self.expr(lower);
                self.expr(upper);
            }
            ExprKind::Form { arguments, .. } => {
                arguments.iter().for_each(|argument| self.expr(argument));
            }
            ExprKind::Struct { path, fields } => {
                self.type_name(&path.segments[0]);
                fields.iter().for_each(|field| self.expr(&field.value));
            }
            ExprKind::Block(block) => self.block(block),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expr(condition);
                self.block(then_branch);
                self.expr(else_branch);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    self.with_pattern(&arm.pattern, |this| this.expr(&arm.body));
                }
            }
            ExprKind::Loop {
                state,
                result,
                body,
            } => {
                result.iter().for_each(|result| self.ty(result));
                self.state(state, |this| this.block(body));
            }
            ExprKind::While {
                pattern,
                condition,
                body,
            } => {
                self.expr(condition);
                match pattern {
                    Some(pattern) => self.with_pattern(pattern, |this| this.block(body)),
                    None => self.block(body),
                }
            }
            ExprKind::For {
                pattern,
                iterable,
                state,
                body,
            } => {
                self.expr(iterable);
                self.with_pattern(pattern, |this| this.state(state, |this| this.block(body)));
            }
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                let names: Vec<String> = parameters
                    .iter()
                    .map(|parameter| parameter.name.text.clone())
                    .collect();
                self.scoped(names, |this| {
                    parameters
                        .iter()
                        .for_each(|parameter| this.ty(&parameter.ty));
                    this.block(body);
                });
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Call { callee, arguments } => {
                match &callee.kind {
                    // A function, or a proposition applied.
                    ExprKind::Name(name) => self.name(name),
                    _ => self.expr(callee),
                }
                arguments.iter().for_each(|argument| self.expr(argument));
            }
            ExprKind::Member { value, .. } | ExprKind::Index { value, .. } => self.expr(value),
        }
    }
}
