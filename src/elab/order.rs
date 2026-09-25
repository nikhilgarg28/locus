//! The order in which declarations are elaborated. An item is checked when it
//! is declared, so everything it mentions must already exist. The core has
//! explicitly checked recursion; mutually recursive logical enums form atomic groups.
//!
//! Dependencies are read off the syntax: every name an item writes that is
//! also the name of an item in the namespace the position reads, types
//! (structs, enums, propositions) or values (functions, constants). A name
//! applied, `Name(..)`, is the function of that name when there is one and
//! the proposition otherwise, as the elaborator reads it. A local shadows
//! a value of its name where it is in scope, as it does for the elaborator:
//! a parameter, a pattern, a loop's state, or a quantifier's variable named
//! like a function is that local, not a mention of the function.
//!
//! The functions of an `impl` block are units of their own (O4): each is
//! named `Type::name` in the value namespace, depends on its type, and reads
//! `Self` as that type. A method call `x.f(..)` names no type, so it is read
//! as a mention of every `f` some `impl` block declares, except that
//! `self.f(..)` names the block's own; the unit itself is left out, and the
//! elaborator reports a method that calls itself through another receiver.

use std::collections::{HashMap, HashSet};

use crate::ast::*;

/// One item to elaborate: a declaration of the file, or one function of an
/// `impl` block with the block's target beside it.
#[derive(Clone, Copy)]
pub(super) struct Unit<'a> {
    pub declaration: &'a Declaration,
    /// The type an `impl` block is for, when the declaration is one of its
    /// functions.
    pub owner: Option<&'a Path>,
    pub model: Option<&'a ModelImpl>,
}

/// The units of a program, in source order: the declarations, with the
/// functions of each `impl` block in the block's place.
pub(super) fn units(program: &Program) -> Vec<Unit<'_>> {
    let mut units = Vec::new();
    for declaration in &program.declarations {
        match &declaration.kind {
            DeclarationKind::Impl {
                target,
                model,
                methods,
                ..
            } => {
                units.extend(methods.iter().map(|method| Unit {
                    declaration: method,
                    owner: Some(target),
                    model: model.as_ref(),
                }));
            }
            _ => units.push(Unit {
                declaration,
                owner: None,
                model: None,
            }),
        }
    }
    units
}

/// The name a unit declares: the item's, or `Type::name` for a function of
/// an `impl` block.
pub(super) fn unit_name(unit: &Unit<'_>) -> Option<String> {
    let name = declared_name(unit.declaration)?;
    Some(match unit.owner {
        Some(owner) => format!("{}::{}", owner.text(), name.text),
        None => name.text.clone(),
    })
}

/// Indices into `units`, dependencies first, and the units that are part
/// of a cycle.
pub(super) fn dependency_order(
    units: &[Unit<'_>],
    quantifiers: &[super::generics::QuantifierNames],
) -> (Vec<Vec<usize>>, Vec<usize>) {
    let names: Vec<Option<String>> = units.iter().map(unit_name).collect();
    let mut index_of: HashMap<(Namespace, &str), usize> = units
        .iter()
        .zip(&names)
        .enumerate()
        .filter_map(|(index, (unit, name))| {
            let namespace = declared_namespace(unit.declaration)?;
            name.as_deref().map(|name| ((namespace, name), index))
        })
        .collect();
    let derived_names: Vec<(String, usize)> = units.iter().enumerate().filter_map(|(index, unit)| {
        if let DeclarationKind::Struct { name, .. } = &unit.declaration.kind
            && unit.declaration.attributes.iter().any(|attribute| matches!(&attribute.kind, AttributeKind::Derive(paths) if paths.iter().any(|path| path.text() == "Model"))) {
            Some((format!("{}Model", name.text), index))
        } else { None }
    }).collect();
    for (name, index) in &derived_names {
        index_of.insert((Namespace::Type, name), *index);
    }
    // Every `impl` function by its own name, for a method call.
    let mut methods: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, unit) in units.iter().enumerate() {
        if unit.owner.is_some()
            && let Some(name) = declared_name(unit.declaration)
        {
            methods.entry(&name.text).or_default().push(index);
        }
    }
    let mut models: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, unit) in units.iter().enumerate() {
        if let Some(model) = unit.model
            && let Some(owner) = unit.owner
        {
            if let Some(source) = model_type_name(&model.source) {
                models.entry(source).or_default().push(index);
            }
            if let Some(source) = model_type_name(&model.source) {
                models
                    .entry(format!("{source}->{}", owner.text()))
                    .or_default()
                    .push(index);
            }
        }
    }
    let edges: Vec<Vec<usize>> = units
        .iter()
        .enumerate()
        .map(|(index, unit)| {
            let mut names = HashSet::new();
            let mut mentions = Mentions {
                names: &mut names,
                bound: Vec::new(),
                owner: unit.owner.map(Path::text),
                logical: matches!(unit.declaration.kind, DeclarationKind::Function { logical: true, .. } | DeclarationKind::Prop { .. }) || unit.declaration.attributes.iter().any(|attribute| matches!(&attribute.kind, AttributeKind::Derive(paths) if paths.iter().any(|path| path.text() == "Model"))),
                model_types: HashSet::new(),
            };
            mentions.unit(unit);
            let logical = mentions.logical;
            let mut model_types = mentions.model_types;
            if logical {
                // A local may get its physical type from a function result,
                // without spelling that type in the observing function.
                let callees: Vec<_> = names.iter().flat_map(|(namespace, name)| {
                    if *namespace == Namespace::Method {
                        methods.get(name.as_str()).into_iter().flatten().copied().collect::<Vec<_>>()
                    } else if matches!(namespace, Namespace::Value | Namespace::Applied) {
                        index_of.get(&(Namespace::Value, name.as_str())).copied().into_iter().collect()
                    } else { Vec::new() }
                }).collect();
                for callee in callees {
                    if let Some(result) = match &units[callee].declaration.kind {
                        DeclarationKind::Function { result, .. } => Some(result),
                        DeclarationKind::Constant { ty, .. } => Some(ty),
                        _ => None,
                    } {
                        let mut unused_names = HashSet::new();
                        let mut result_mentions = Mentions {
                            names: &mut unused_names,
                            bound: Vec::new(),
                            owner: units[callee].owner.map(Path::text),
                            logical: true,
                            model_types: HashSet::new(),
                        };
                        result_mentions.ty(result);
                        model_types.extend(result_mentions.model_types);
                    }
                }
                for name in model_types { names.insert((Namespace::Model, name)); }
            }
            let mut edges: Vec<usize> = names
                .iter()
                .flat_map(|(namespace, name)| match namespace {
                    Namespace::Applied => index_of
                        .get(&(Namespace::Value, name.as_str()))
                        .or_else(|| index_of.get(&(Namespace::Type, name.as_str())))
                        .copied()
                        .into_iter()
                        .collect::<Vec<usize>>(),
                    Namespace::Model => {
                        if unit.model.is_some_and(|model| model_type_name(&model.source).as_ref() == Some(name)) {
                            Vec::new()
                        } else { models.get(name).into_iter().flatten().copied().filter(|found| *found != index).collect() }
                    },
                    Namespace::Method => methods
                        .get(name.as_str())
                        .into_iter()
                        .flatten()
                        .copied()
                        .filter(|found| *found != index)
                        .collect(),
                    read => index_of
                        .get(&(*read, name.as_str()))
                        .copied()
                        .into_iter()
                        .collect(),
                })
                .collect();
            if matches!(unit.declaration.kind, DeclarationKind::Enum { .. })
                || matches!(unit.declaration.kind, DeclarationKind::Function { logical: true, .. })
                || matches!(&unit.declaration.kind, DeclarationKind::Prop { variants, .. } if variants.iter().all(|variant| variant.body.is_some()))
            {
                edges.retain(|edge| *edge != index);
            }
            for pair in quantifiers {
                let exists = index_of.get(&(Namespace::Type, pair.exists.as_str())).copied();
                let forall = index_of.get(&(Namespace::Type, pair.forall.as_str())).copied();
                if let (Some(exists), Some(forall)) = (exists, forall)
                    && index != exists && index != forall && (edges.contains(&exists) || edges.contains(&forall)) {
                        edges.extend([exists, forall]);
                    }
            }
            edges.sort_unstable();
            edges.dedup();
            edges
        })
        .collect();

    // Collapse only checked logical-data components. Other declaration
    // cycles retain the ordinary cycle diagnostic and are never forward
    // declared in the kernel.
    let mut representative: Vec<_> = (0..units.len()).collect();
    let mut groups: Vec<Vec<usize>> = (0..units.len()).map(|i| vec![i]).collect();
    for mut group in strongly_connected(&edges) {
        if group.len() > 1
            && group.iter().all(|index| {
                matches!(units[*index].declaration.kind, DeclarationKind::Enum { .. })
                    && super::logical_data::has_logical_derive(
                        &units[*index].declaration.attributes,
                    )
            })
        {
            group.sort_unstable();
            let first = group[0];
            for &member in &group {
                representative[member] = first;
            }
            groups[first] = group;
        }
    }
    let grouped_edges: Vec<Vec<usize>> = (0..units.len())
        .map(|index| {
            if representative[index] != index {
                return Vec::new();
            }
            let mut targets: Vec<_> = groups[index]
                .iter()
                .flat_map(|member| edges[*member].iter())
                .map(|target| representative[*target])
                .filter(|target| groups[index].len() == 1 || *target != index)
                .collect();
            targets.sort_unstable();
            targets.dedup();
            targets
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
    for (node, &group) in representative.iter().enumerate() {
        if group == node {
            visit(node, &grouped_edges, &mut state, &mut order, &mut cyclic);
        }
    }
    order.retain(|node| !cyclic.contains(node));
    (
        order
            .into_iter()
            .map(|index| groups[index].clone())
            .collect(),
        cyclic,
    )
}

/// Strongly connected components, found without recursive graph traversal.
fn strongly_connected(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; edges.len()];
    let mut finished = Vec::new();
    for start in 0..edges.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![(start, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if let Some(&target) = edges[*node].get(*next) {
                *next += 1;
                if !seen[target] {
                    seen[target] = true;
                    stack.push((target, 0));
                }
            } else {
                finished.push(*node);
                stack.pop();
            }
        }
    }
    let mut incoming = vec![Vec::new(); edges.len()];
    for (source, targets) in edges.iter().enumerate() {
        for &target in targets {
            incoming[target].push(source);
        }
    }
    seen.fill(false);
    let mut groups = Vec::new();
    for start in finished.into_iter().rev() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut stack = vec![start];
        let mut group = Vec::new();
        while let Some(node) = stack.pop() {
            group.push(node);
            for &target in &incoming[node] {
                if !seen[target] {
                    seen[target] = true;
                    stack.push(target);
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// Which of Rust's two namespaces a name is read in; `Applied` is a call,
/// `Name(..)`, which is the value when there is one and the type otherwise;
/// `Method` is a call `x.f(..)`, which is every `f` an `impl` block has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Namespace {
    Type,
    Value,
    Applied,
    Method,
    Model,
}

/// The namespace an item declares its name in. An `impl` block declares
/// none of its own.
fn declared_namespace(declaration: &Declaration) -> Option<Namespace> {
    match &declaration.kind {
        DeclarationKind::Foreign { .. }
        | DeclarationKind::Function { .. }
        | DeclarationKind::Constant { .. } => Some(Namespace::Value),
        DeclarationKind::Struct { .. }
        | DeclarationKind::Enum { .. }
        | DeclarationKind::Prop { .. } => Some(Namespace::Type),
        DeclarationKind::Impl { .. }
        | DeclarationKind::SpecImpl { .. }
        | DeclarationKind::AssociatedType { .. }
        | DeclarationKind::Spec { .. }
        | DeclarationKind::ModuleImpl { .. }
        | DeclarationKind::Module { .. }
        | DeclarationKind::Use { .. }
        | DeclarationKind::ImportedModule { .. }
        | DeclarationKind::RustImport { .. } => None,
    }
}

/// The name an item declares. An `impl` block declares none of its own.
pub(super) fn declared_name(declaration: &Declaration) -> Option<&Name> {
    match &declaration.kind {
        DeclarationKind::Foreign { name, .. }
        | DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(name),
        DeclarationKind::Impl { .. }
        | DeclarationKind::SpecImpl { .. }
        | DeclarationKind::AssociatedType { .. }
        | DeclarationKind::Spec { .. }
        | DeclarationKind::ModuleImpl { .. }
        | DeclarationKind::Module { .. }
        | DeclarationKind::Use { .. }
        | DeclarationKind::ImportedModule { .. }
        | DeclarationKind::RustImport { .. } => None,
    }
}

struct Mentions<'a> {
    names: &'a mut HashSet<(Namespace, String)>,
    /// The locals in scope, innermost last.
    bound: Vec<String>,
    /// Inside an `impl` block: its target, which `Self` names.
    owner: Option<String>,
    logical: bool,
    model_types: HashSet<String>,
}

impl Mentions<'_> {
    /// A type's name as written, with `Self` read as the block's target.
    fn type_text(&self, name: &Name) -> String {
        match (&self.owner, name.text.as_str()) {
            (Some(owner), "Self") => owner.clone(),
            _ => name.text.clone(),
        }
    }

    /// A name where a type is read: a type, the prefix of a path, a struct
    /// literal or pattern. No local shadows a type.
    fn type_name(&mut self, name: &Name) {
        let text = self.type_text(name);
        self.model_types.insert(text.clone());
        self.names.insert((Namespace::Type, text));
    }

    /// `Prefix::name` where a value is read: the function of an `impl`
    /// block of that name, when there is one, beside the type the prefix
    /// names.
    fn path(&mut self, path: &Path) {
        self.type_name(&path.segments[0]);
        if let Some((prefix, name)) = path.pair() {
            let qualified = format!("{}::{}", self.type_text(prefix), name.text);
            self.names.insert((Namespace::Value, qualified));
        }
    }

    /// `receiver.name(..)`: the block's own method when the receiver is
    /// `self`, and every method of that name otherwise.
    fn method(&mut self, receiver: &Expr, name: &Name) {
        match (&self.owner, &receiver.kind) {
            (Some(owner), ExprKind::Name(written)) if written.text == "self" => {
                let qualified = format!("{owner}::{}", name.text);
                self.names.insert((Namespace::Value, qualified));
            }
            _ => {
                self.names.insert((Namespace::Method, name.text.clone()));
            }
        }
    }

    /// A unit: its declaration, and for a function of an `impl` block the
    /// block's type, which `self` has.
    fn unit(&mut self, unit: &Unit<'_>) {
        if let Some(owner) = unit.owner {
            self.type_name(&owner.segments[0]);
        }
        self.declaration(unit.declaration);
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
            PatternKind::Binding { name, pattern, .. } => {
                self.bound.push(name.text.clone());
                self.bind(pattern);
            }
            PatternKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.bind(constructor);
                self.bind(evidence);
            }
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
                            if let Some(body) = &variant.body {
                                this.block(body);
                            }
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
            // Its functions are units of their own.
            DeclarationKind::Impl { .. }
            | DeclarationKind::SpecImpl { .. }
            | DeclarationKind::AssociatedType { .. }
            | DeclarationKind::Spec { .. }
            | DeclarationKind::ModuleImpl { .. }
            | DeclarationKind::Module { .. }
            | DeclarationKind::Use { .. }
            | DeclarationKind::ImportedModule { .. }
            | DeclarationKind::RustImport { .. }
            | DeclarationKind::Foreign { .. } => {}
        }
    }

    fn ty(&mut self, ty: &Type) {
        if let Some(name) = model_type_name(ty)
            && name != "Buffer"
        {
            self.model_types.insert(name);
        }
        if matches!(
            &ty.kind,
            TypeKind::Proof(_) | TypeKind::LogicalFunction { .. }
        ) || matches!(&ty.kind, TypeKind::Named(name) if matches!(name.text.as_str(), "Int" | "Nat" | "Bool" | "Prop"))
        {
            self.logical = true;
        }
        match &ty.kind {
            TypeKind::Scoped { name, claims } => {
                self.type_name(name);
                claims.iter().for_each(|claim| self.expr(claim));
            }
            TypeKind::Named(name) => self.type_name(name),
            TypeKind::Path { path, arguments } => {
                self.type_name(&path.segments[0]);
                arguments.iter().for_each(|argument| self.ty(argument));
            }
            TypeKind::Lifetime(_) | TypeKind::Unit => {}
            TypeKind::Group(inner) | TypeKind::Slice(inner) => self.ty(inner),
            TypeKind::Array { element, length } => {
                self.ty(element);
                self.expr(length);
            }
            // A field's type may mention the fields before it.
            TypeKind::Tuple(fields) => self.fields(fields, |_| {}),
            TypeKind::Proof(proposition) => self.expr(proposition),
            TypeKind::Function {
                parameters, result, ..
            }
            | TypeKind::LogicalFunction { parameters, result } => {
                self.fields(parameters, |this| this.ty(result))
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
            PatternKind::Binding { pattern, .. } => self.pattern(pattern),
            PatternKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.pattern(constructor);
                self.pattern(evidence);
            }
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

    /// A pattern's names are in scope in `body`.
    fn with_pattern(&mut self, pattern: &Pattern, body: impl FnOnce(&mut Self)) {
        let depth = self.bound.len();
        self.pattern(pattern);
        self.bind(pattern);
        body(self);
        self.bound.truncate(depth);
    }

    fn expr(&mut self, expr: &Expr) {
        if matches!(
            &expr.kind,
            ExprKind::Logic(_)
                | ExprKind::Forall { .. }
                | ExprKind::Exists { .. }
                | ExprKind::Form {
                    form: Form::Model | Form::Prop | Form::Prove | Form::Fold | Form::Unfold,
                    ..
                }
        ) {
            self.logical = true;
        }
        match &expr.kind {
            ExprKind::Scoped { value, ty, .. } => {
                self.expr(value);
                self.ty(ty);
            }
            ExprKind::Subscript { value, index } => {
                self.expr(value);
                self.expr(index);
            }
            ExprKind::Name(name) => self.value_name(name),
            ExprKind::Path(path) => self.path(path),
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
                expr: inner,
                ty,
                source_hint,
                ..
            } => {
                self.expr(inner);
                self.ty(ty);
                if let Some(target) = model_type_name(ty) {
                    let dependency = source_hint
                        .as_deref()
                        .and_then(model_type_name)
                        .map_or_else(|| target.clone(), |source| format!("{source}->{target}"));
                    self.names.insert((Namespace::Model, dependency));
                }
            }
            ExprKind::Array(items) | ExprKind::Tuple(items) => {
                items.iter().for_each(|item| self.expr(item))
            }
            ExprKind::Continue => {}
            ExprKind::Range { lower, upper, .. } => {
                self.expr(lower);
                self.expr(upper);
            }
            ExprKind::Form {
                form,
                arguments,
                source_hint,
                ..
            } => {
                arguments.iter().for_each(|argument| self.expr(argument));
                if *form == Form::Model
                    && let Some(ty) = source_hint.as_deref()
                {
                    self.ty(ty);
                    if let Some(source) = model_type_name(ty) {
                        self.names.insert((Namespace::Model, source));
                    }
                }
            }
            ExprKind::Struct { path, fields } => {
                self.type_name(&path.segments[0]);
                fields.iter().for_each(|field| self.expr(&field.value));
            }
            ExprKind::Logic(block) | ExprKind::Block(block) => self.block(block),
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
                if matches!(scrutinee.kind, ExprKind::Ref { mutable: false, .. }) {
                    for arm in arms {
                        if let PatternKind::Variant { path, .. }
                        | PatternKind::Struct { path, .. } = &arm.pattern.kind
                        {
                            self.model_types.remove(&path.segments[0].text);
                        }
                    }
                }
            }
            ExprKind::Loop { body } => self.block(body),
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
                body,
            } => {
                self.expr(iterable);
                self.with_pattern(pattern, |this| this.block(body));
            }
            ExprKind::Closure { parameters, body } => {
                let names: Vec<_> = parameters
                    .iter()
                    .map(|parameter| parameter.name.text.clone())
                    .collect();
                self.scoped(names, |this| {
                    for parameter in parameters {
                        this.ty(&parameter.ty);
                    }
                    this.expr(body);
                });
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
            ExprKind::GenericApply { callee, arguments } => {
                self.expr(callee);
                arguments.iter().for_each(|argument| self.ty(argument));
            }
            ExprKind::Evidence {
                constructor: left,
                evidence: right,
                ..
            }
            | ExprKind::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            ExprKind::Call { callee, arguments } => {
                match &callee.kind {
                    // A function, or a proposition applied.
                    ExprKind::Name(name) => self.name(name),
                    // A method, whose receiver is read as well.
                    ExprKind::Member { value, name } => {
                        self.method(value, name);
                        self.expr(value);
                    }
                    _ => self.expr(callee),
                }
                arguments.iter().for_each(|argument| self.expr(argument));
            }
            ExprKind::Member { value, .. } | ExprKind::Index { value, .. } => self.expr(value),
        }
    }
}

fn model_type_name(ty: &Type) -> Option<String> {
    match &ty.kind {
        TypeKind::Named(name) => Some(name.text.clone()),
        TypeKind::Path { path, .. } if path.text() == "Vec" => Some("Buffer".into()),
        TypeKind::Path { path, .. } => Some(path.text()),
        TypeKind::Array { .. } | TypeKind::Slice(_) => Some("Buffer".into()),
        TypeKind::Tuple(_) => Some("Tuple".into()),
        TypeKind::Group(inner) | TypeKind::Ref { inner, .. } => model_type_name(inner),
        _ => None,
    }
}
