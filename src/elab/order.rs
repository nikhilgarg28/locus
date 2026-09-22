//! The order in which declarations are elaborated. An item is checked when it
//! is declared, so everything it mentions must already exist. The core has
//! no recursion, so a cycle is an error.
//!
//! Dependencies are read off the syntax: every name an item writes that is
//! also the name of an item. A local that shadows an item's name counts as a
//! mention of it, which can only add an edge.

use std::collections::{HashMap, HashSet};

use crate::ast::*;

/// Indices into `program.declarations`, dependencies first, and the
/// declarations that are part of a cycle.
pub(super) fn dependency_order(program: &Program) -> (Vec<usize>, Vec<usize>) {
    let index_of: HashMap<&str, usize> = program
        .declarations
        .iter()
        .enumerate()
        .filter_map(|(index, declaration)| {
            declared_name(declaration).map(|name| (name.text.as_str(), index))
        })
        .collect();
    let edges: Vec<Vec<usize>> = program
        .declarations
        .iter()
        .map(|declaration| {
            let mut names = HashSet::new();
            Mentions(&mut names).declaration(declaration);
            let mut edges: Vec<usize> = names
                .iter()
                .filter_map(|name| index_of.get(name.as_str()).copied())
                .collect();
            edges.sort_unstable();
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

struct Mentions<'a>(&'a mut HashSet<String>);

impl Mentions<'_> {
    fn name(&mut self, name: &Name) {
        self.0.insert(name.text.clone());
    }

    fn declaration(&mut self, declaration: &Declaration) {
        match &declaration.kind {
            DeclarationKind::Function {
                parameters,
                result,
                body,
                ..
            } => {
                parameters
                    .iter()
                    .for_each(|parameter| self.ty(&parameter.ty));
                self.ty(result);
                self.block(body);
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
                parameters
                    .iter()
                    .for_each(|parameter| self.ty(&parameter.ty));
                for variant in variants {
                    variant.fields.iter().for_each(|field| self.ty(&field.ty));
                    // `: @Name(arguments)` names the proposition being
                    // declared, which is not a use of it.
                    if let Some(ExprKind::Call { arguments, .. }) =
                        variant.target.as_ref().map(|target| &target.kind)
                    {
                        arguments.iter().for_each(|argument| self.expr(argument));
                    }
                }
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
            TypeKind::Named(name) => self.name(name),
            TypeKind::Path { path, arguments } => {
                self.name(&path.segments[0]);
                arguments.iter().for_each(|argument| self.ty(argument));
            }
            TypeKind::Unit => {}
            TypeKind::Group(inner) => self.ty(inner),
            TypeKind::Tuple(fields) => fields.iter().for_each(|field| self.ty(&field.ty)),
            TypeKind::Proof(proposition) => self.expr(proposition),
            TypeKind::Function {
                parameters, result, ..
            } => {
                parameters.iter().for_each(|field| self.ty(&field.ty));
                self.ty(result);
            }
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
                self.name(&path.segments[0]);
                fields.iter().for_each(|field| self.pattern(&field.pattern));
            }
            PatternKind::Variant { path, arguments } => {
                self.name(&path.segments[0]);
                for argument in arguments.iter().flatten() {
                    self.pattern(argument);
                }
            }
        }
    }

    fn block(&mut self, block: &Block) {
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
    }

    fn state(&mut self, state: &[StateParameter]) {
        for parameter in state {
            self.ty(&parameter.ty);
            self.expr(&parameter.initial);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Name(name) => self.name(name),
            ExprKind::Path(path) => self.name(&path.segments[0]),
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
                self.name(&path.segments[0]);
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
                    self.pattern(&arm.pattern);
                    self.expr(&arm.body);
                }
            }
            ExprKind::Loop {
                state,
                result,
                body,
            } => {
                self.state(state);
                result.iter().for_each(|result| self.ty(result));
                self.block(body);
            }
            ExprKind::While {
                pattern,
                condition,
                body,
            } => {
                pattern.iter().for_each(|pattern| self.pattern(pattern));
                self.expr(condition);
                self.block(body);
            }
            ExprKind::For {
                pattern,
                iterable,
                state,
                body,
            } => {
                self.pattern(pattern);
                self.expr(iterable);
                self.state(state);
                self.block(body);
            }
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                parameters
                    .iter()
                    .for_each(|parameter| self.ty(&parameter.ty));
                self.block(body);
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Call { callee, arguments } => {
                self.expr(callee);
                arguments.iter().for_each(|argument| self.expr(argument));
            }
            ExprKind::Member { value, .. } | ExprKind::Index { value, .. } => self.expr(value),
        }
    }
}
