use crate::{ast::*, diagnostic::Diagnostic, source::Span};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Namespace {
    Type,
    Value,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Module(usize),
    Item(usize),
}
#[derive(Clone, Debug)]
pub struct Binding {
    pub target: Target,
    pub visibility: Option<Visibility>,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub struct Scope {
    pub parent: Option<usize>,
    pub name: String,
    pub span: Span,
    pub bindings: BTreeMap<(String, Namespace), Binding>,
    imports: Vec<(Import, Option<Visibility>, Span)>,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub module: usize,
    pub canonical: String,
    pub original: String,
    pub namespace: Namespace,
    pub declaration: Declaration,
}
#[derive(Clone, Debug)]
pub struct Export {
    pub path: Vec<String>,
    pub item: usize,
    pub span: Span,
}
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub scopes: Vec<Scope>,
    pub items: Vec<Item>,
    pub access: Access,
    pub export_root: usize,
    pub package_roots: Vec<usize>,
    pub export_roots: Vec<Vec<usize>>,
    pub externs: BTreeMap<usize, BTreeMap<String, usize>>,
}
#[derive(Clone, Debug, Default)]
pub struct Access {
    pub scopes: Vec<(Option<usize>, Span)>,
    pub names: Vec<String>,
}
impl Access {
    pub fn module_at(&self, span: Span) -> usize {
        self.scopes
            .iter()
            .enumerate()
            .filter(|(_, (_, s))| s.file == span.file && s.start <= span.start && s.end >= span.end)
            .min_by_key(|(_, (_, s))| s.end - s.start)
            .map_or(0, |(i, _)| i)
    }
    pub fn descendant(&self, mut module: usize, ancestor: usize) -> bool {
        loop {
            if module == ancestor {
                return true;
            }
            match self.scopes[module].0 {
                Some(p) => module = p,
                None => return false,
            }
        }
    }
    fn root(&self, mut module: usize) -> usize {
        while let Some(parent) = self.scopes[module].0 {
            module = parent;
        }
        module
    }
    fn restriction(&self, owner: usize, path: &Path) -> Option<usize> {
        let mut parts = path.segments.iter();
        let mut module = match parts.next()?.text.as_str() {
            "crate" => self.root(owner),
            "self" => owner,
            "super" => self.scopes[owner].0?,
            _ => return None,
        };
        for part in parts {
            if part.text == "super" {
                module = self.scopes[module].0?;
            } else {
                module = self
                    .scopes
                    .iter()
                    .enumerate()
                    .find(|(i, (parent, _))| {
                        *parent == Some(module) && self.names[*i] == part.text
                    })?
                    .0;
            }
        }
        self.descendant(owner, module).then_some(module)
    }
    fn extent(&self, visibility: Option<&Visibility>, owner: usize) -> Option<usize> {
        match visibility.map(|v| &v.scope) {
            Some(VisibilityScope::Public) => None,
            Some(VisibilityScope::Crate) => Some(self.root(owner)),
            Some(VisibilityScope::Super) => Some(self.scopes[owner].0.unwrap_or(owner)),
            Some(VisibilityScope::In(path)) => Some(self.restriction(owner, path).unwrap_or(owner)),
            _ => Some(owner),
        }
    }
    pub fn allowed(&self, visibility: Option<&Visibility>, owner: usize, requester: usize) -> bool {
        match visibility.map(|v| &v.scope) {
            Some(VisibilityScope::Public) => true,
            Some(VisibilityScope::Crate) => self.root(owner) == self.root(requester),
            Some(VisibilityScope::Super) => {
                self.descendant(requester, self.scopes[owner].0.unwrap_or(owner))
            }
            Some(VisibilityScope::In(path)) => self
                .restriction(owner, path)
                .is_some_and(|scope| self.descendant(requester, scope)),
            _ => self.descendant(requester, owner),
        }
    }
}

pub struct Unit {
    pub program: Program,
    pub name: String,
    pub dependencies: BTreeMap<String, usize>,
    pub exports: Vec<Option<Program>>,
}

pub fn resolve(program: Program) -> (Program, Graph, Vec<Diagnostic>) {
    resolve_units(
        vec![Unit {
            program,
            name: "crate".into(),
            dependencies: BTreeMap::new(),
            exports: Vec::new(),
        }],
        None,
    )
}
pub fn resolve_units(
    units: Vec<Unit>,
    selected: Option<usize>,
) -> (Program, Graph, Vec<Diagnostic>) {
    let mut graph = Graph::default();
    let mut errors = Vec::new();
    let mut dependencies = Vec::new();
    for unit in units {
        let span = program_span(&unit.program);
        let root = graph.collect(unit.program, None, unit.name, span, &mut errors);
        graph.package_roots.push(root);
        dependencies.push(unit.dependencies);
        let mut exports = Vec::new();
        for (i, program) in unit.exports.into_iter().enumerate() {
            if let Some(program) = program {
                let span = program_span(&program);
                exports.push(graph.collect(
                    program,
                    Some(root),
                    format!("__export{i}"),
                    span,
                    &mut errors,
                ));
            } else {
                exports.push(root);
            }
        }
        graph.export_roots.push(exports);
    }
    graph.export_root = selected.map_or(graph.package_roots[0], |i| graph.export_roots[0][i]);
    for (i, deps) in dependencies.into_iter().enumerate() {
        let root = graph.package_roots[i];
        graph.externs.insert(
            root,
            deps.into_iter()
                .map(|(name, index)| (name, graph.package_roots[index]))
                .collect(),
        );
    }
    graph.access.scopes = graph.scopes.iter().map(|s| (s.parent, s.span)).collect();
    graph.access.names = graph.scopes.iter().map(|s| s.name.clone()).collect();
    graph.validate_visibility(&mut errors);
    // The root contains every source span, including empty units.
    graph.access.scopes[0].1.start = 0;
    graph.access.scopes[0].1.end = usize::MAX;
    graph.imports(&mut errors);
    graph.export_cycles(&mut errors);
    let mut declarations = Vec::new();
    for index in 0..graph.items.len() {
        let item = &graph.items[index];
        let mut d = item.declaration.clone();
        let mut rewrite = Rewriter {
            graph: &graph,
            module: item.module,
            values: Vec::new(),
            types: Vec::new(),
            errors: &mut errors,
        };
        rewrite.declaration(&mut d);
        if let Some(name) = declared_name_mut(&mut d.kind) {
            name.text = item.canonical.clone();
        }
        declarations.push(d);
    }
    (
        Program {
            declarations,
            ..Program::default()
        },
        graph,
        errors,
    )
}
fn program_span(program: &Program) -> Span {
    program
        .declarations
        .first()
        .map(|d| d.span.through(program.declarations.last().unwrap().span))
        .unwrap_or(Span::new(crate::source::FileId(0), 0, 0))
}
impl Graph {
    pub fn package_of(&self, module: usize) -> usize {
        let root = self.access.root(module);
        self.package_roots.iter().position(|r| *r == root).unwrap()
    }
    fn validate_visibility(&self, errors: &mut Vec<Diagnostic>) {
        fn check(g: &Graph, v: Option<&Visibility>, module: usize, errors: &mut Vec<Diagnostic>) {
            if let Some(Visibility {
                scope: VisibilityScope::In(path),
                span,
            }) = v
                && g.access.restriction(module, path).is_none()
            {
                errors.push(Diagnostic::error("L0503","visibility must name this module or a lexical ancestor using crate, self or super",*span));
            }
        }
        for (m, s) in self.scopes.iter().enumerate() {
            for b in s.bindings.values() {
                check(self, b.visibility.as_ref(), m, errors);
            }
        }
        for item in &self.items {
            check(
                self,
                item.declaration.visibility.as_ref(),
                item.module,
                errors,
            );
            match &item.declaration.kind {
                DeclarationKind::Struct { fields, .. } => {
                    for f in fields {
                        check(self, f.visibility.as_ref(), item.module, errors);
                    }
                }
                DeclarationKind::Impl { methods, .. } => {
                    for method in methods {
                        check(self, method.visibility.as_ref(), item.module, errors);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect(
        &mut self,
        program: Program,
        parent: Option<usize>,
        name: String,
        span: Span,
        errors: &mut Vec<Diagnostic>,
    ) -> usize {
        let module = self.scopes.len();
        self.scopes.push(Scope {
            parent,
            name,
            span,
            bindings: BTreeMap::new(),
            imports: Vec::new(),
        });
        for mut d in program.declarations {
            // File/module promises affect this module's declarations, not its children.
            if matches!(d.kind, DeclarationKind::Function { .. }) {
                d.attributes.splice(0..0, program.attributes.clone());
            }
            if let DeclarationKind::Impl { methods, .. } = &mut d.kind {
                for method in methods {
                    method.attributes.splice(0..0, program.attributes.clone());
                }
            }
            match d.kind.clone() {
                DeclarationKind::Module {
                    name,
                    body: Some(body),
                } => {
                    if !d.attributes.is_empty() {
                        errors.push(Diagnostic::error("L0500","attributes on modules are not supported; put promises in the module body",d.span));
                    }
                    let child = self.collect(body, Some(module), name.text.clone(), d.span, errors);
                    self.bind(
                        module,
                        name.text,
                        Namespace::Type,
                        Binding {
                            target: Target::Module(child),
                            visibility: d.visibility,
                            span: d.span,
                        },
                        errors,
                    );
                }
                DeclarationKind::Module { body: None, .. } => errors.push(Diagnostic::error(
                    "L0501",
                    "external module was not loaded",
                    d.span,
                )),
                DeclarationKind::Use { imports } => {
                    if !d.attributes.is_empty() {
                        errors.push(Diagnostic::error(
                            "L0500",
                            "attributes on imports are not supported",
                            d.span,
                        ));
                    }
                    for import in imports {
                        self.scopes[module]
                            .imports
                            .push((import, d.visibility.clone(), d.span));
                    }
                }
                _ => {
                    let ns = if matches!(
                        d.kind,
                        DeclarationKind::Struct { .. }
                            | DeclarationKind::Enum { .. }
                            | DeclarationKind::Prop { .. }
                    ) {
                        Namespace::Type
                    } else {
                        Namespace::Value
                    };
                    let name = declared_name(&d.kind)
                        .map(|n| n.text.clone())
                        .unwrap_or_default();
                    let id = self.items.len();
                    let canonical = if ns == Namespace::Type {
                        format!("LocusM{module}N{id}{name}")
                    } else {
                        format!("__locus_m{module}_n{id}_{name}")
                    };
                    self.items.push(Item {
                        module,
                        canonical,
                        original: name.clone(),
                        namespace: ns,
                        declaration: d.clone(),
                    });
                    if !name.is_empty() {
                        self.bind(
                            module,
                            name,
                            ns,
                            Binding {
                                target: Target::Item(id),
                                visibility: d.visibility,
                                span: d.span,
                            },
                            errors,
                        );
                    }
                }
            }
        }
        module
    }
    fn bind(
        &mut self,
        module: usize,
        name: String,
        ns: Namespace,
        binding: Binding,
        errors: &mut Vec<Diagnostic>,
    ) {
        if let Some(first) = self.scopes[module].bindings.get(&(name.clone(), ns)) {
            errors.push(
                Diagnostic::error(
                    "L0502",
                    format!("`{name}` is bound twice in this module"),
                    binding.span,
                )
                .label(first.span, "first binding here"),
            );
        } else {
            self.scopes[module].bindings.insert((name, ns), binding);
        }
    }
    fn imports(&mut self, errors: &mut Vec<Diagnostic>) {
        let mut pending: Vec<_> = self
            .scopes
            .iter()
            .enumerate()
            .flat_map(|(m, s)| s.imports.iter().cloned().map(move |i| (m, i)))
            .collect();
        loop {
            let mut next = Vec::new();
            let mut progress = false;
            for (module, (import, visibility, span)) in pending {
                let mut found = false;
                for ns in [Namespace::Type, Namespace::Value] {
                    if let Ok((target, tail)) = self.lookup(module, &import.path, ns)
                        && tail.is_empty()
                    {
                        found = true;
                        let (owner, target_vis) = match target {
                            Target::Module(m) => match self.scopes[m].parent {
                                Some(parent) => (
                                    parent,
                                    self.scopes[parent]
                                        .bindings
                                        .get(&(self.scopes[m].name.clone(), Namespace::Type))
                                        .and_then(|b| b.visibility.as_ref()),
                                ),
                                None => (m, None),
                            },
                            Target::Item(i) => (
                                self.items[i].module,
                                self.items[i].declaration.visibility.as_ref(),
                            ),
                        };
                        let target_scope = self.access.extent(target_vis, owner);
                        let alias_scope = self.access.extent(visibility.as_ref(), module);
                        let widens = match (target_scope, alias_scope) {
                            (None, _) => false,
                            (Some(_), None) => true,
                            (Some(target), Some(alias)) => !self.access.descendant(alias, target),
                        };
                        let invalid=visibility.as_ref().is_some_and(|v|matches!(&v.scope,VisibilityScope::In(path) if self.access.restriction(module,path).is_none()));
                        if widens || invalid {
                            errors.push(Diagnostic::error("L0503","an import cannot widen its target's visibility and must name a valid ancestor restriction",span).label(import.path.span,"restricted target"));
                        }
                        let name = import
                            .alias
                            .as_ref()
                            .unwrap_or(import.path.last())
                            .text
                            .clone();
                        self.bind(
                            module,
                            name,
                            ns,
                            Binding {
                                target,
                                visibility: visibility.clone(),
                                span,
                            },
                            errors,
                        );
                    }
                }
                if found {
                    progress = true;
                } else {
                    next.push((module, (import, visibility, span)));
                }
            }
            if next.is_empty() {
                break;
            }
            if !progress {
                for (module, (import, _, span)) in next {
                    let d = self
                        .lookup(module, &import.path, Namespace::Value)
                        .err()
                        .filter(|d| d.code == "L0503")
                        .or_else(|| self.lookup(module, &import.path, Namespace::Type).err())
                        .unwrap_or_else(|| Diagnostic::error("L0502", "unresolved import", span));
                    errors.push(d.note("check the path and visibility; cyclic re-exports without a definition cannot resolve"));
                }
                break;
            }
            pending = next;
        }
    }
    pub fn lookup(
        &self,
        requester: usize,
        path: &Path,
        ns: Namespace,
    ) -> Result<(Target, Vec<Name>), Diagnostic> {
        let segments = &path.segments;
        let mut module = requester;
        let mut at = 0;
        if segments[0].text == "crate" {
            module = self.access.root(requester);
            at = 1;
        } else if segments[0].text == "self" {
            at = 1;
        }
        while at < segments.len() && segments[at].text == "super" {
            module = self.scopes[module].parent.ok_or_else(|| {
                Diagnostic::error(
                    "L0502",
                    "`super` goes above the source root",
                    segments[at].span,
                )
            })?;
            at += 1;
        }
        if at == segments.len() {
            return Ok((Target::Module(module), Vec::new()));
        }
        while at < segments.len() {
            let name = &segments[at];
            let final_segment = at + 1 == segments.len();
            let binding = self.scopes[module].bindings.get(&(
                name.text.clone(),
                if final_segment { ns } else { Namespace::Type },
            ));
            if binding.is_none()
                && at == 0
                && let Some(&dependency) = self
                    .externs
                    .get(&self.access.root(requester))
                    .and_then(|d| d.get(&name.text))
            {
                module = dependency;
                at += 1;
                if at == segments.len() {
                    return Ok((Target::Module(module), Vec::new()));
                }
                continue;
            }
            let Some(binding) = binding else {
                return Err(Diagnostic::error(
                    "L0502",
                    format!(
                        "cannot resolve `{}` in module `{}`",
                        name.text, self.scopes[module].name
                    ),
                    name.span,
                ));
            };
            if !self
                .access
                .allowed(binding.visibility.as_ref(), module, requester)
            {
                return Err(Diagnostic::error(
                    "L0503",
                    format!("`{}` is private to its module", name.text),
                    name.span,
                )
                .label(binding.span, "declared here"));
            }
            match binding.target {
                Target::Module(m) => {
                    module = m;
                    at += 1;
                    if at == segments.len() {
                        return Ok((binding.target, Vec::new()));
                    }
                }
                Target::Item(_) => return Ok((binding.target, segments[at + 1..].to_vec())),
            }
        }
        unreachable!()
    }
    fn export_cycles(&self, errors: &mut Vec<Diagnostic>) {
        fn visit(
            g: &Graph,
            m: usize,
            active: &mut BTreeSet<usize>,
            done: &mut BTreeSet<usize>,
            errors: &mut Vec<Diagnostic>,
        ) {
            if !done.insert(m) {
                return;
            }
            active.insert(m);
            for b in g.scopes[m].bindings.values() {
                if is_public(b.visibility.as_ref())
                    && let Target::Module(next) = b.target
                {
                    if active.contains(&next) {
                        errors.push(Diagnostic::error("L0502","cyclic public module re-exports cannot be represented by a finite export facade",b.span));
                    } else {
                        visit(g, next, active, done, errors);
                    }
                }
            }
            active.remove(&m);
        }
        let mut done = BTreeSet::new();
        for m in 0..self.scopes.len() {
            visit(self, m, &mut BTreeSet::new(), &mut done, errors);
        }
    }
    pub fn exports(&self, module: usize) -> Vec<Export> {
        fn visit(
            g: &Graph,
            m: usize,
            prefix: &[String],
            ancestors: &mut BTreeSet<usize>,
            out: &mut Vec<Export>,
        ) {
            if !ancestors.insert(m) {
                return;
            }
            for ((name, _), b) in &g.scopes[m].bindings {
                if !is_public(b.visibility.as_ref()) {
                    continue;
                }
                let mut path = prefix.to_vec();
                path.push(name.clone());
                match b.target {
                    Target::Module(m) => visit(g, m, &path, ancestors, out),
                    Target::Item(item) => out.push(Export {
                        path,
                        item,
                        span: b.span,
                    }),
                }
            }
            ancestors.remove(&m);
        }
        let mut out = Vec::new();
        visit(self, module, &[], &mut BTreeSet::new(), &mut out);
        out
    }
    pub fn display_name(&self, canonical: &str) -> String {
        self.items
            .iter()
            .find(|i| i.canonical == canonical)
            .map_or_else(|| canonical.into(), |i| i.original.clone())
    }
}
pub fn is_public(v: Option<&Visibility>) -> bool {
    matches!(v.map(|v| &v.scope), Some(VisibilityScope::Public))
}
pub fn declared_name(k: &DeclarationKind) -> Option<&Name> {
    match k {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(name),
        _ => None,
    }
}
fn declared_name_mut(k: &mut DeclarationKind) -> Option<&mut Name> {
    match k {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(name),
        _ => None,
    }
}

struct Rewriter<'a> {
    graph: &'a Graph,
    module: usize,
    values: Vec<String>,
    types: Vec<String>,
    errors: &'a mut Vec<Diagnostic>,
}
impl Rewriter<'_> {
    fn path(&mut self, path: &mut Path, ns: Namespace) {
        if path.segments.len() == 1
            && (self.types.contains(&path.segments[0].text)
                || (ns == Namespace::Value && self.values.contains(&path.segments[0].text)))
        {
            return;
        }
        if path
            .segments
            .iter()
            .any(|n| n.text.starts_with("__locus_") || n.text.starts_with("LocusM"))
        {
            self.errors.push(Diagnostic::error(
                "L0502",
                "this identifier prefix is reserved for generated module identities",
                path.span,
            ));
            return;
        }
        if path.segments[0].text == "Self"
            || (path.segments.len() == 1 && path.segments[0].text == "self")
        {
            return;
        }
        let result = self.graph.lookup(self.module, path, ns).or_else(|e| {
            if ns == Namespace::Value && e.code != "L0503" {
                self.graph.lookup(self.module, path, Namespace::Type)
            } else {
                Err(e)
            }
        });
        match result {
            Ok((Target::Item(i), tail)) => {
                let mut name = path.segments[0].clone();
                name.text = self.graph.items[i].canonical.clone();
                path.segments = vec![name];
                path.segments.extend(tail);
            }
            Ok((Target::Module(_), _)) => self.errors.push(Diagnostic::error(
                "L0502",
                "a module is not a value or type",
                path.span,
            )),
            Err(d) => {
                // Unqualified unknown names and built-in type prefixes are diagnosed by elaboration.
                if path.segments.len() > 1 && !builtin(&path.segments[0].text) || d.code == "L0503"
                {
                    self.errors.push(d);
                }
            }
        }
    }
    fn name(&mut self, name: &mut Name, ns: Namespace) {
        let mut p = Path {
            segments: vec![name.clone()],
            span: name.span,
        };
        self.path(&mut p, ns);
        *name = p.segments.remove(0);
    }
    fn declaration(&mut self, d: &mut Declaration) {
        let old_v = self.values.len();
        let old_t = self.types.len();
        match &mut d.kind {
            DeclarationKind::Function {
                generics,
                parameters,
                result,
                body,
                ..
            } => {
                self.types
                    .extend(generics.iter().map(|g| g.name.text.clone()));
                for p in parameters {
                    self.ty(&mut p.ty);
                    self.values.push(p.name.text.clone());
                }
                self.ty(result);
                for a in &mut d.attributes {
                    if let AttributeKind::Terminates { decreases: Some(e) } = &mut a.kind {
                        self.expr(e);
                    }
                }
                self.block(body);
            }
            DeclarationKind::Struct {
                generics, fields, ..
            } => {
                self.types
                    .extend(generics.iter().map(|g| g.name.text.clone()));
                for f in fields {
                    self.ty(&mut f.ty);
                    self.values.push(f.name.text.clone());
                }
            }
            DeclarationKind::Enum {
                generics, variants, ..
            } => {
                self.types
                    .extend(generics.iter().map(|g| g.name.text.clone()));
                for v in variants {
                    let n = self.values.len();
                    self.fields(&mut v.fields);
                    self.values.truncate(n);
                }
            }
            DeclarationKind::Prop {
                generics,
                parameters,
                variants,
                ..
            } => {
                self.types
                    .extend(generics.iter().map(|g| g.name.text.clone()));
                for p in parameters {
                    self.ty(&mut p.ty);
                    self.values.push(p.name.text.clone());
                }
                for v in variants {
                    let n = self.values.len();
                    self.fields(&mut v.fields);
                    if let Some(e) = &mut v.target {
                        self.expr(e);
                    }
                    if let Some(b) = &mut v.body {
                        self.block(b);
                    }
                    self.values.truncate(n);
                }
            }
            DeclarationKind::Constant { ty, value, .. } => {
                self.ty(ty);
                self.expr(value);
            }
            DeclarationKind::Impl {
                target,
                model,
                methods,
            } => {
                if let Ok((Target::Item(i), _)) =
                    self.graph.lookup(self.module, target, Namespace::Type)
                    && self.graph.package_of(self.graph.items[i].module)
                        != self.graph.package_of(self.module)
                {
                    self.errors.push(Diagnostic::error("L0503","an inherent or Model implementation must belong to the type's Cargo package",target.span));
                }
                self.path(target, Namespace::Type);
                if let Some(m) = model {
                    self.ty(&mut m.source);
                    self.ty(&mut m.target);
                }
                for m in methods {
                    self.declaration(m);
                }
            }
            _ => {}
        }
        self.values.truncate(old_v);
        self.types.truncate(old_t);
    }
    fn fields(&mut self, fields: &mut [TypeField]) {
        for f in fields {
            self.ty(&mut f.ty);
            if let Some(n) = &f.name {
                self.values.push(n.text.clone());
            }
        }
    }
    fn ty(&mut self, t: &mut Type) {
        match &mut t.kind {
            TypeKind::Named(n) => self.name(n, Namespace::Type),
            TypeKind::Path { path, arguments } => {
                self.path(path, Namespace::Type);
                for a in arguments.iter_mut() {
                    self.ty(a);
                }
                // A qualified monomorphic name becomes the same node as an
                // imported name after resolution, not a zero-argument application.
                if arguments.is_empty() && path.segments.len() == 1 {
                    t.kind = TypeKind::Named(path.segments[0].clone());
                }
            }
            TypeKind::Group(t) | TypeKind::Slice(t) | TypeKind::Ref { inner: t, .. } => self.ty(t),
            TypeKind::Array { element, length } => {
                self.ty(element);
                self.expr(length);
            }
            TypeKind::Tuple(fs) => {
                let n = self.values.len();
                self.fields(fs);
                self.values.truncate(n);
            }
            TypeKind::Proof(e) => self.expr(e),
            TypeKind::Function { parameters, result }
            | TypeKind::LogicalFunction { parameters, result } => {
                let n = self.values.len();
                self.fields(parameters);
                self.ty(result);
                self.values.truncate(n);
            }
            _ => {}
        }
    }
    fn block(&mut self, b: &mut Block) {
        let n = self.values.len();
        for s in &mut b.statements {
            match &mut s.kind {
                StatementKind::Let {
                    pattern,
                    annotation,
                    value,
                    ..
                } => {
                    if let Some(t) = annotation {
                        self.ty(t);
                    }
                    self.expr(value);
                    self.pattern(pattern);
                }
                StatementKind::Assign { place, value } => {
                    self.expr(place);
                    self.expr(value);
                }
                StatementKind::Expression(e) => self.expr(e),
                StatementKind::Error => {}
            }
        }
        if let Some(e) = &mut b.tail {
            self.expr(e);
        }
        self.values.truncate(n);
    }
    fn pattern(&mut self, p: &mut Pattern) {
        match &mut p.kind {
            PatternKind::Name { name, .. } => self.values.push(name.text.clone()),
            PatternKind::Binding { name, pattern, .. } => {
                self.values.push(name.text.clone());
                self.pattern(pattern);
            }
            PatternKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.pattern(constructor);
                self.pattern(evidence);
            }
            PatternKind::Group(p) => self.pattern(p),
            PatternKind::Tuple(ps) => {
                for p in ps {
                    self.pattern(p);
                }
            }
            PatternKind::Struct { path, fields, .. } => {
                self.path(path, Namespace::Type);
                for f in fields {
                    self.pattern(&mut f.pattern);
                }
            }
            PatternKind::Variant { path, arguments } => {
                self.path(path, Namespace::Type);
                if let Some(ps) = arguments {
                    for p in ps {
                        self.pattern(p);
                    }
                }
            }
            _ => {}
        }
    }
    fn expr(&mut self, e: &mut Expr) {
        match &mut e.kind {
            ExprKind::Name(n) => self.name(n, Namespace::Value),
            ExprKind::Path(p) => {
                self.path(p, Namespace::Value);
                if p.segments.len() == 1 {
                    e.kind = ExprKind::Name(p.segments[0].clone());
                }
            }
            ExprKind::Struct { path, fields } => {
                self.path(path, Namespace::Type);
                for f in fields {
                    if f.name.is_none()
                        && let ExprKind::Name(n) = &f.value.kind
                    {
                        f.name = Some(n.clone());
                    }
                    self.expr(&mut f.value);
                }
            }
            ExprKind::Group(e)
            | ExprKind::Not(e)
            | ExprKind::Unary { expr: e, .. }
            | ExprKind::Ref { expr: e, .. } => self.expr(e),
            ExprKind::Tuple(es) | ExprKind::Array(es) | ExprKind::Form { arguments: es, .. } => {
                for e in es {
                    self.expr(e);
                }
            }
            ExprKind::Subscript { value, index } => {
                self.expr(value);
                self.expr(index);
            }
            ExprKind::Block(b) | ExprKind::Logic(b) | ExprKind::Loop { body: b } => self.block(b),
            ExprKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.expr(constructor);
                self.expr(evidence);
            }
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
                for a in arms {
                    let n = self.values.len();
                    self.pattern(&mut a.pattern);
                    self.expr(&mut a.body);
                    self.values.truncate(n);
                }
            }
            ExprKind::While {
                pattern,
                condition,
                body,
            } => {
                self.expr(condition);
                let n = self.values.len();
                if let Some(p) = pattern {
                    self.pattern(p);
                }
                self.block(body);
                self.values.truncate(n);
            }
            ExprKind::For {
                pattern,
                iterable,
                body,
            } => {
                self.expr(iterable);
                let n = self.values.len();
                self.pattern(pattern);
                self.block(body);
                self.values.truncate(n);
            }
            ExprKind::Range { lower, upper, .. }
            | ExprKind::Binary {
                left: lower,
                right: upper,
                ..
            } => {
                self.expr(lower);
                self.expr(upper);
            }
            ExprKind::Return(e) | ExprKind::Break(e) => {
                if let Some(e) = e {
                    self.expr(e);
                }
            }
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                let n = self.values.len();
                for p in parameters {
                    self.ty(&mut p.ty);
                    self.values.push(p.name.text.clone());
                }
                self.block(body);
                self.values.truncate(n);
            }
            ExprKind::Cast {
                expr,
                ty,
                source_hint,
                ..
            } => {
                self.expr(expr);
                self.ty(ty);
                if let Some(t) = source_hint {
                    self.ty(t);
                }
            }
            ExprKind::Call { callee, arguments } => {
                self.expr(callee);
                for a in arguments {
                    self.expr(a);
                }
            }
            ExprKind::Closure { parameters, body } => {
                let n = self.values.len();
                for p in parameters {
                    self.ty(&mut p.ty);
                    self.values.push(p.name.text.clone());
                }
                self.expr(body);
                self.values.truncate(n);
            }
            ExprKind::GenericApply { callee, arguments } => {
                self.expr(callee);
                for a in arguments {
                    self.ty(a);
                }
            }
            ExprKind::Member { value, .. } | ExprKind::Index { value, .. } => self.expr(value),
            _ => {}
        }
    }
}
fn builtin(s: &str) -> bool {
    matches!(
        s,
        "u8" | "u16"
            | "u32"
            | "u64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "Int"
            | "Bool"
            | "Nat"
            | "Vec"
            | "Box"
            | "Option"
            | "Result"
            | "True"
            | "False"
            | "Eq"
            | "And"
            | "Or"
            | "ForAll"
            | "Exists"
    )
}
