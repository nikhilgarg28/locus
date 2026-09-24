//! Surface generics are checked templates. Every concrete instantiation is
//! expanded into ordinary AST items and goes through the normal elaborator,
//! lowering and kernel checker. Unused bodies are not claimed to be universally
//! checked. There is no proof- or type-checking bypass in this pass.
use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::*;
use crate::diagnostic::Diagnostic;
use crate::preview::{Feature, Previews};
use crate::source::{SourceMap, Span};

/// Limits on *distinct* instances and on nesting within one type expression.
/// Growing polymorphic recursion is rejected, not allowed to hang the compiler.
use crate::limits::MAX_GENERIC_INSTANCES;
use crate::limits::MAX_GENERIC_TYPE_DEPTH;
type Types = HashMap<String, Type>;

#[derive(Clone)]
pub(super) struct QuantifierNames {
    pub element: Type,
    pub exists: String,
    pub forall: String,
}

pub(super) fn specialize(
    program: &Program,
    previews: &Previews,
) -> (Program, Vec<Diagnostic>, Vec<QuantifierNames>) {
    let mut pass = Specializer {
        previews,
        templates: HashMap::new(),
        declarations: HashMap::new(),
        instances: HashMap::new(),
        origins: HashMap::new(),
        queue: VecDeque::new(),
        names: HashSet::new(),
        diagnostics: crate::limits::DiagnosticBuffer::default(),
        depth: 0,
        return_type: None,
        model_serial: 0,
        quantifier_templates: HashSet::new(),
    };
    for declaration in &program.declarations {
        pass.register(declaration.clone(), false);
    }
    // Associated functions participate in expected-type propagation just as
    // free functions do, even when their impl block appears later in the file.
    for declaration in &program.declarations {
        if let DeclarationKind::Impl {
            target,
            model: None,
            methods,
        } = &declaration.kind
        {
            let self_type = Type {
                span: target.span,
                kind: TypeKind::Path {
                    path: Box::new(target.clone()),
                    arguments: vec![],
                },
            };
            for method in methods {
                if let Some(name) = declaration_name(method) {
                    pass.declarations.insert(
                        format!("{}::{}", target.text(), name.text),
                        (
                            method.clone(),
                            Types::from([("Self".into(), self_type.clone())]),
                        ),
                    );
                }
            }
        }
    }
    // These are ordinary checked enum templates, not trusted declarations.
    let mut sources = SourceMap::default();
    let id = sources.add(
        "<generic prelude>",
        "pub enum Option<T> { None, Some(T) } pub enum Result<T, E> { Ok(T), Err(E) }
        prop Exists<T: Logical>(predicate: logic Fn(value: T) -> Prop) {
            Witness(value: T) => { predicate(value) }
        }
        prop ForAll<T: Logical>(predicate: logic Fn(value: T) -> Prop) {
            Each(prove_each: logic Fn(value: T) -> @predicate(value)) => { prop!(true) }
        }",
    );
    let parsed = crate::parser::parse(sources.get(id));
    for mut declaration in parsed.program.declarations {
        if let Some(span) = program.declarations.first().map(|d| d.span) {
            declaration.span = span;
        }
        let name = declaration_name(&declaration).unwrap().text.clone();
        if !pass.names.contains(&name) {
            if matches!(name.as_str(), "Exists" | "ForAll") {
                pass.quantifier_templates.insert(name);
            }
            pass.register(declaration, true);
        }
    }
    let mut declarations = Vec::new();
    // Only source monomorphic declarations enter the initial worklist.
    for declaration in &program.declarations {
        if parameters(declaration).is_empty() {
            pass.queue.push_back((declaration.clone(), Types::new()));
        }
    }
    while let Some((mut declaration, substitutions)) = pass.queue.pop_front() {
        pass.declaration(&mut declaration, &substitutions);
        if let Some(name) = declaration_name(&declaration) {
            pass.declarations
                .insert(name.text.clone(), (declaration.clone(), Types::new()));
        }
        declarations.push(declaration);
    }
    let mut quantifiers = Vec::new();
    if pass.quantifier_templates.contains("Exists") && pass.quantifier_templates.contains("ForAll")
    {
        for (name, (template, arguments)) in &pass.origins {
            if template == "Exists"
                && arguments.len() == 1
                && let Some((forall, _)) = pass.origins.iter().find(|(_, (template, args))| {
                    template == "ForAll"
                        && args.len() == 1
                        && type_key(&args[0]) == type_key(&arguments[0])
                })
            {
                quantifiers.push(QuantifierNames {
                    element: arguments[0].clone(),
                    exists: name.clone(),
                    forall: forall.clone(),
                });
            }
        }
        quantifiers.sort_by(|a, b| a.exists.cmp(&b.exists));
    }
    (
        Program {
            declarations,
            ..program.clone()
        },
        pass.diagnostics.into_vec(),
        quantifiers,
    )
}

struct Specializer<'a> {
    previews: &'a Previews,
    templates: HashMap<String, Declaration>,
    declarations: HashMap<String, (Declaration, Types)>,
    instances: HashMap<String, String>,
    origins: HashMap<String, (String, Vec<Type>)>,
    queue: VecDeque<(Declaration, Types)>,
    names: HashSet<String>,
    diagnostics: crate::limits::DiagnosticBuffer,
    depth: usize,
    return_type: Option<Type>,
    model_serial: usize,
    quantifier_templates: HashSet<String>,
}

impl Specializer<'_> {
    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error("L0281", message, span));
    }

    fn gate(&mut self, span: Span) -> bool {
        match self.previews.require(
            Feature::LogicalData,
            "generic declaration or application",
            span,
        ) {
            Ok(()) => true,
            Err(diagnostic) => {
                self.diagnostics.push(diagnostic);
                false
            }
        }
    }

    fn register(&mut self, declaration: Declaration, builtin: bool) {
        let Some(name) = declaration_name(&declaration) else {
            return;
        };
        let name = name.text.clone();
        self.names.insert(name.clone());
        if all_parameters(&declaration)
            .iter()
            .any(|parameter| parameter.lifetime)
            && let Err(diagnostic) =
                self.previews
                    .require(Feature::HeapViews, "lifetime parameters", declaration.span)
        {
            self.diagnostics.push(diagnostic);
        }
        let params = parameters(&declaration);
        if params.is_empty() {
            self.declarations.insert(name, (declaration, Types::new()));
            return;
        }
        if !builtin {
            self.gate(declaration.span);
        }
        let mut seen = HashSet::new();
        for parameter in params {
            if !seen.insert(parameter.name.text.clone()) {
                self.error("a generic parameter is declared twice", parameter.span);
            }
            for bound in &parameter.bounds {
                if bound.text() != "Logical" {
                    self.error("only the `Logical` generic bound is supported", bound.span);
                }
            }
            if derives_logical(&declaration)
                && parameter.bounds.is_empty()
                && stores_parameter(&declaration, &parameter.name.text)
            {
                self.error(
                    "a stored type parameter in a logical aggregate needs the `Logical` bound",
                    parameter.span,
                );
            }
        }
        let span = declaration.span;
        if let std::collections::hash_map::Entry::Vacant(entry) = self.templates.entry(name.clone())
        {
            entry.insert(declaration);
        } else {
            self.error(format!("generic `{name}` is declared twice"), span);
        }
    }

    fn instantiate(
        &mut self,
        template: &str,
        mut args: Vec<Type>,
        substitutions: &Types,
        locals: &Types,
        span: Span,
    ) -> Option<String> {
        if !self.gate(span) {
            return None;
        }
        args.retain(|argument| !matches!(argument.kind, TypeKind::Lifetime(_)));
        let Some(mut declaration) = self.templates.get(template).cloned() else {
            self.error(format!("`{template}` is not a generic declaration"), span);
            return None;
        };
        let params = parameters(&declaration);
        if params.len() != args.len() {
            self.error(
                format!(
                    "`{template}` needs {} type arguments, got {}",
                    params.len(),
                    args.len()
                ),
                span,
            );
            return None;
        }
        for argument in &mut args {
            self.ty(argument, substitutions, locals);
            if has_open_proof(argument, locals) {
                self.error("a generic type argument captures a local value; open dependent type arguments are not supported yet", argument.span);
                return None;
            }
        }
        for (parameter, argument) in params.iter().zip(&args) {
            if !parameter.bounds.is_empty() && !self.logical(argument) {
                self.error(
                    format!(
                        "type argument for `{}` does not satisfy `Logical`",
                        parameter.name.text
                    ),
                    argument.span,
                );
                return None;
            }
        }
        let key = format!(
            "{template}<{}>",
            args.iter().map(type_key).collect::<Vec<_>>().join(",")
        );
        if let Some(name) = self.instances.get(&key) {
            return Some(name.clone());
        }
        if self.instances.len() >= MAX_GENERIC_INSTANCES {
            self.error(format!("generic specialization exceeds MAX_GENERIC_INSTANCES limit of {MAX_GENERIC_INSTANCES} distinct instances"), span);
            return None;
        }
        let mut serial = self.instances.len();
        let name = loop {
            let name = if matches!(declaration.kind, DeclarationKind::Function { .. }) {
                format!("__locus_{template}_{serial}")
            } else {
                let stem = template
                    .split('_')
                    .filter(|part| !part.is_empty())
                    .map(|part| {
                        let mut chars = part.chars();
                        chars
                            .next()
                            .unwrap()
                            .to_uppercase()
                            .chain(chars)
                            .collect::<String>()
                    })
                    .collect::<String>();
                format!("__Locus{stem}{serial}")
            };
            if self.names.insert(name.clone()) {
                break name;
            }
            serial += 1;
        };
        self.instances.insert(key, name.clone());
        self.origins
            .insert(name.clone(), (template.to_owned(), args.clone()));
        let substitutions: Types = params
            .iter()
            .zip(args.clone())
            .map(|(parameter, argument)| (parameter.name.text.clone(), argument))
            .collect();
        rename_instance(&mut declaration, &name);
        self.declarations
            .insert(name.clone(), (declaration.clone(), substitutions.clone()));
        self.queue.push_back((declaration, substitutions));
        if self.quantifier_templates.contains(template) {
            let companion = if template == "Exists" {
                "ForAll"
            } else {
                "Exists"
            };
            if self.quantifier_templates.contains(companion) {
                self.instantiate(companion, args, &Types::new(), &Types::new(), span);
            }
        }
        Some(name)
    }

    fn logical(&self, ty: &Type) -> bool {
        match &ty.kind {
            TypeKind::Named(name) => {
                if matches!(name.text.as_str(), "Int" | "Nat" | "Bool" | "Prop" | "Unit") {
                    return true;
                }
                self.declarations
                    .get(&name.text)
                    .is_some_and(|(d, _)| derives_logical(d))
            }
            TypeKind::Group(inner) => self.logical(inner),
            TypeKind::Proof(_) | TypeKind::LogicalFunction { .. } => true,
            TypeKind::Tuple(fields) => {
                !fields.is_empty() && fields.iter().all(|field| self.logical(&field.ty))
            }
            _ => false,
        }
    }

    fn declaration(&mut self, declaration: &mut Declaration, substitutions: &Types) {
        let mut locals = Types::new();
        match &mut declaration.kind {
            DeclarationKind::Module { .. } | DeclarationKind::Use { .. } => {
                self.diagnostics.push(Diagnostic::error(
                    "L0500",
                    "modules require the project loader",
                    declaration.span,
                ));
            }
            DeclarationKind::Function {
                parameters,
                result,
                body,
                ..
            } => {
                for parameter in parameters {
                    self.ty(&mut parameter.ty, substitutions, &locals);
                    locals.insert(parameter.name.text.clone(), parameter.ty.clone());
                }
                self.ty(result, substitutions, &locals);
                let previous = self.return_type.replace(result.clone());
                self.block(body, Some(result), substitutions, &mut locals);
                self.return_type = previous;
            }
            DeclarationKind::Struct { fields, .. } => {
                for field in fields {
                    self.ty(&mut field.ty, substitutions, &locals);
                    locals.insert(field.name.text.clone(), field.ty.clone());
                }
            }
            DeclarationKind::Enum { variants, .. } => {
                for variant in variants {
                    self.fields(&mut variant.fields, substitutions, &mut Types::new());
                }
            }
            DeclarationKind::Prop {
                parameters,
                variants,
                ..
            } => {
                for parameter in parameters {
                    self.ty(&mut parameter.ty, substitutions, &locals);
                    locals.insert(parameter.name.text.clone(), parameter.ty.clone());
                }
                for variant in variants {
                    let mut locals = locals.clone();
                    self.fields(&mut variant.fields, substitutions, &mut locals);
                    if let Some(target) = &mut variant.target {
                        self.expr(target, None, substitutions, &locals);
                    }
                    if let Some(body) = &mut variant.body {
                        self.block(body, None, substitutions, &mut locals);
                    }
                }
            }
            DeclarationKind::Constant { ty, value, .. } => {
                self.ty(ty, substitutions, &locals);
                self.expr(value, Some(ty), substitutions, &locals);
            }
            DeclarationKind::Impl {
                target,
                model,
                methods,
            } => {
                let mut substitutions = substitutions.clone();
                if let Some(model) = model {
                    self.gate(model.span);
                    self.ty(&mut model.source, &substitutions, &locals);
                    self.ty(&mut model.target, &substitutions, &locals);
                    if let Some(name) = type_name(&model.target) {
                        target.segments = vec![name.clone()];
                        target.span = model.target.span;
                    }
                    substitutions.insert("Self".into(), model.target.clone());
                    if methods.len() == 1
                        && let DeclarationKind::Function { name, .. } = &mut methods[0].kind
                        && name.text == "model"
                    {
                        name.text = format!("__model_{}", self.model_serial);
                        self.model_serial += 1;
                    }
                }
                for method in methods {
                    if !parameters(method).is_empty() {
                        self.error("generic methods in impl blocks are not supported yet; use a generic free function", method.span);
                    }
                    self.declaration(method, &substitutions);
                }
            }
        }
        for attribute in &mut declaration.attributes {
            if let AttributeKind::Terminates {
                decreases: Some(expr),
            } = &mut attribute.kind
            {
                self.expr(expr, None, substitutions, &locals);
            }
        }
    }

    fn fields(&mut self, fields: &mut [TypeField], substitutions: &Types, locals: &mut Types) {
        for field in fields {
            self.ty(&mut field.ty, substitutions, locals);
            if let Some(name) = &field.name {
                locals.insert(name.text.clone(), field.ty.clone());
            }
        }
    }

    fn ty(&mut self, ty: &mut Type, substitutions: &Types, locals: &Types) {
        if self.depth >= MAX_GENERIC_TYPE_DEPTH {
            self.error(
                format!("generic type nesting exceeds MAX_GENERIC_TYPE_DEPTH limit of {MAX_GENERIC_TYPE_DEPTH}"),
                ty.span,
            );
            return;
        }
        self.depth += 1;
        match &mut ty.kind {
            TypeKind::Named(name) => {
                if let Some(replacement) = substitutions.get(&name.text) {
                    *ty = replacement.clone();
                } else if self.templates.contains_key(&name.text) {
                    self.error(
                        format!("generic type `{}` requires type arguments", name.text),
                        ty.span,
                    );
                }
            }
            TypeKind::Path { path, arguments }
                if !arguments.is_empty() && self.templates.contains_key(&path.text()) =>
            {
                if let Some(name) = self.instantiate(
                    &path.text(),
                    arguments.clone(),
                    substitutions,
                    locals,
                    ty.span,
                ) {
                    let lifetimes: Vec<_> = arguments
                        .iter()
                        .filter(|argument| matches!(argument.kind, TypeKind::Lifetime(_)))
                        .cloned()
                        .collect();
                    let name = Name {
                        text: name,
                        span: ty.span,
                    };
                    ty.kind = if lifetimes.is_empty() {
                        TypeKind::Named(name)
                    } else {
                        TypeKind::Path {
                            path: Box::new(Path {
                                segments: vec![name],
                                span: ty.span,
                            }),
                            arguments: lifetimes,
                        }
                    };
                }
            }
            TypeKind::Path { arguments, .. } => {
                for argument in arguments {
                    self.ty(argument, substitutions, locals);
                }
            }
            TypeKind::Group(inner) | TypeKind::Ref { inner, .. } | TypeKind::Slice(inner) => {
                self.ty(inner, substitutions, locals)
            }
            TypeKind::Array { element, length } => {
                self.ty(element, substitutions, locals);
                self.expr(length, None, substitutions, locals);
            }
            TypeKind::Tuple(fields) => self.fields(fields, substitutions, &mut locals.clone()),
            TypeKind::Proof(proposition) => {
                self.expr(proposition, None, substitutions, locals);
            }
            TypeKind::Function { parameters, result }
            | TypeKind::LogicalFunction { parameters, result } => {
                let mut locals = locals.clone();
                self.fields(parameters, substitutions, &mut locals);
                self.ty(result, substitutions, &locals);
            }
            TypeKind::Lifetime(_) | TypeKind::Unit | TypeKind::Never => {}
        }
        self.depth -= 1;
    }

    fn block(
        &mut self,
        block: &mut Block,
        expected: Option<&Type>,
        substitutions: &Types,
        locals: &mut Types,
    ) -> Option<Type> {
        for statement in &mut block.statements {
            match &mut statement.kind {
                StatementKind::Let {
                    pattern,
                    annotation,
                    value,
                    ..
                } => {
                    if let Some(annotation) = annotation {
                        self.ty(annotation, substitutions, locals);
                    }
                    let inferred = self.expr(value, annotation.as_ref(), substitutions, locals);
                    self.pattern(
                        pattern,
                        annotation.as_ref().or(inferred.as_ref()),
                        substitutions,
                        locals,
                    );
                }
                StatementKind::Assign { place, value } => {
                    let ty = self.expr(place, None, substitutions, locals);
                    self.expr(value, ty.as_ref(), substitutions, locals);
                }
                StatementKind::Expression(expr) => {
                    self.expr(expr, None, substitutions, locals);
                }
                StatementKind::Error => {}
            }
        }
        block
            .tail
            .as_mut()
            .and_then(|tail| self.expr(tail, expected, substitutions, locals))
    }

    fn specialize_path(
        &mut self,
        path: &mut Path,
        args: Option<Vec<Type>>,
        expected: Option<&Type>,
        substitutions: &Types,
        locals: &Types,
    ) {
        let Some(first) = path.segments.first() else {
            return;
        };
        let template = first.text.clone();
        if !self.templates.contains_key(&template) {
            return;
        }
        let args = args.or_else(|| expected.and_then(|ty| self.expected_instance(ty, &template)));
        let Some(args) = args else {
            self.error(format!("cannot infer type arguments for `{template}`; write `::<...>` or provide an expected type"), path.span);
            return;
        };
        if let Some(name) = self.instantiate(&template, args, substitutions, locals, path.span) {
            path.segments[0].text = name;
        }
    }

    fn expected_instance(&self, expected: &Type, template: &str) -> Option<Vec<Type>> {
        match &expected.kind {
            TypeKind::Group(inner) => self.expected_instance(inner, template),
            TypeKind::Path { path, arguments }
                if arguments
                    .iter()
                    .all(|arg| matches!(arg.kind, TypeKind::Lifetime(_))) =>
            {
                self.origins
                    .get(&path.text())
                    .filter(|(origin, _)| origin == template)
                    .map(|(_, args)| args.clone())
            }
            TypeKind::Named(name) => self
                .origins
                .get(&name.text)
                .filter(|(origin, _)| origin == template)
                .map(|(_, args)| args.clone()),
            TypeKind::Proof(proposition) => match &proposition.kind {
                ExprKind::Name(name) => self.origins.get(&name.text),
                ExprKind::Call { callee, .. } => match &callee.kind {
                    ExprKind::Name(name) => self.origins.get(&name.text),
                    _ => None,
                },
                _ => None,
            }
            .filter(|(origin, _)| origin == template)
            .map(|(_, args)| args.clone()),
            _ => None,
        }
    }

    fn signature(&mut self, name: &str, locals: &Types) -> Option<(Vec<Type>, Type)> {
        let (declaration, substitutions) = self.declarations.get(name)?.clone();
        match declaration.kind {
            DeclarationKind::Prop { parameters, .. } => {
                let mut context = locals.clone();
                let mut inputs = Vec::new();
                for mut parameter in parameters {
                    self.ty(&mut parameter.ty, &substitutions, &context);
                    context.insert(parameter.name.text, parameter.ty.clone());
                    inputs.push(parameter.ty);
                }
                Some((
                    inputs,
                    named_type(&Name {
                        text: "Prop".into(),
                        span: declaration.span,
                    }),
                ))
            }
            DeclarationKind::Function {
                parameters,
                mut result,
                ..
            } => {
                let mut context = locals.clone();
                let mut inputs = Vec::new();
                for mut parameter in parameters {
                    self.ty(&mut parameter.ty, &substitutions, &context);
                    context.insert(parameter.name.text, parameter.ty.clone());
                    inputs.push(parameter.ty);
                }
                self.ty(&mut result, &substitutions, &context);
                Some((inputs, result))
            }
            _ => None,
        }
    }

    fn payload(&mut self, path: &Path, locals: &Types) -> Option<Vec<TypeField>> {
        let (declaration, substitutions) = self.declarations.get(&path.segments[0].text)?.clone();
        let mut fields = match declaration.kind {
            DeclarationKind::Struct { fields, .. } => fields
                .into_iter()
                .map(|field| TypeField {
                    name: Some(field.name),
                    ty: field.ty,
                    span: field.span,
                })
                .collect(),
            DeclarationKind::Enum { variants, .. } => {
                variants
                    .into_iter()
                    .find(|variant| variant.name.text == path.last().text)?
                    .fields
            }
            DeclarationKind::Prop { variants, .. } => {
                variants
                    .into_iter()
                    .find(|variant| variant.name.text == path.last().text)?
                    .fields
            }
            _ => return None,
        };
        self.fields(&mut fields, &substitutions, &mut locals.clone());
        Some(fields)
    }

    fn infer_arguments(
        &mut self,
        name: &str,
        arguments: &mut [Expr],
        expected: Option<&Type>,
        substitutions: &Types,
        locals: &Types,
    ) -> Option<Vec<Type>> {
        let declaration = self.templates.get(name)?.clone();
        let (generics, parameters, result) = match declaration.kind {
            DeclarationKind::Function {
                generics,
                parameters,
                result,
                ..
            } => (generics, parameters, result),
            DeclarationKind::Prop {
                generics,
                parameters,
                ..
            } => (
                generics,
                parameters,
                named_type(&Name {
                    text: "Prop".into(),
                    span: declaration.span,
                }),
            ),
            _ => return None,
        };
        let generics: Vec<_> = generics
            .iter()
            .filter(|parameter| !parameter.lifetime)
            .collect();
        let names: HashSet<_> = generics.iter().map(|p| p.name.text.clone()).collect();
        let mut inferred = Types::new();
        if let Some(expected) = expected {
            infer_type_arguments(&result, expected, &names, &mut inferred, &self.origins);
        }
        for (argument, parameter) in arguments.iter_mut().zip(&parameters) {
            if let Some(actual) = self.expr(argument, None, substitutions, locals) {
                infer_type_arguments(&parameter.ty, &actual, &names, &mut inferred, &self.origins);
            }
        }
        generics
            .iter()
            .map(|parameter| inferred.get(&parameter.name.text).cloned())
            .collect()
    }

    fn expr(
        &mut self,
        expr: &mut Expr,
        expected: Option<&Type>,
        substitutions: &Types,
        locals: &Types,
    ) -> Option<Type> {
        {
            if let ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } =
                &expr.kind
            {
                if let Some(statement) = body.statements.first() {
                    self.diagnostics.push(Diagnostic::error(
                        "L0290",
                        "statements inside a quantifier are not supported yet",
                        statement.span,
                    ));
                    return None;
                }
                if body.tail.is_none() {
                    self.diagnostics.push(Diagnostic::error(
                        "L0222",
                        "a quantifier needs a formula to state",
                        body.span,
                    ));
                    return None;
                }
                let universal = matches!(expr.kind, ExprKind::Forall { .. });
                *expr = quantifier_expression(universal, parameters, body, expr.span);
            }
        }
        // Eliminate the parser's explicit application wrapper, retaining all
        // value arguments and their source evaluation order unchanged.
        if let ExprKind::GenericApply { callee, arguments } = &expr.kind {
            let mut callee = *callee.clone();
            let arguments = arguments.clone();
            if let ExprKind::Name(name) = &mut callee.kind
                && let Some(instance) = self.instantiate(
                    &name.text,
                    arguments.clone(),
                    substitutions,
                    locals,
                    expr.span,
                )
            {
                name.text = instance;
            }
            // Struct paths and boxed path expressions have different storage.
            match &mut callee.kind {
                ExprKind::Path(path) => self.specialize_path(
                    path,
                    Some(arguments.clone()),
                    expected,
                    substitutions,
                    locals,
                ),
                ExprKind::Struct { path, .. } => self.specialize_path(
                    path,
                    Some(arguments.clone()),
                    expected,
                    substitutions,
                    locals,
                ),
                ExprKind::Name(_) => {}
                _ => self.error(
                    "explicit type arguments require a named declaration",
                    expr.span,
                ),
            }
            *expr = callee;
        }
        if let ExprKind::Name(name) = &expr.kind {
            let parent = match name.text.as_str() {
                "Some" | "None" => Some("Option"),
                "Ok" | "Err" => Some("Result"),
                _ => None,
            };
            if let Some(parent) = parent
                .filter(|_| !self.names.contains(&name.text) && !locals.contains_key(&name.text))
            {
                let mut path = Path {
                    segments: vec![
                        Name {
                            text: parent.into(),
                            span: name.span,
                        },
                        name.clone(),
                    ],
                    span: expr.span,
                };
                self.specialize_path(&mut path, None, expected, substitutions, locals);
                expr.kind = ExprKind::Path(Box::new(path));
            }
        }
        let span = expr.span;
        let inferred = match &mut expr.kind {
            ExprKind::Name(name) => {
                if self.templates.contains_key(&name.text) {
                    let mut path = Path {
                        segments: vec![name.clone()],
                        span,
                    };
                    self.specialize_path(&mut path, None, expected, substitutions, locals);
                    *name = path.segments.remove(0);
                }
                locals.get(&name.text).cloned().or_else(|| {
                    self.declarations
                        .get(&name.text)
                        .and_then(|(d, _)| match &d.kind {
                            DeclarationKind::Constant { ty, .. } => Some(ty.clone()),
                            _ => None,
                        })
                })
            }
            ExprKind::Path(path) => {
                self.specialize_path(path, None, expected, substitutions, locals);
                self.payload(path, locals)
                    .map(|_| named_type(&path.segments[0]))
            }
            ExprKind::Integer(literal) => expected.cloned().or_else(|| {
                literal.suffix.map(|suffix| Type {
                    kind: TypeKind::Named(Name {
                        text: suffix.name().into(),
                        span,
                    }),
                    span,
                })
            }),
            ExprKind::Bool(_) => expected.cloned().or_else(|| {
                Some(Type {
                    kind: TypeKind::Named(Name {
                        text: "bool".into(),
                        span,
                    }),
                    span,
                })
            }),
            ExprKind::Unit => Some(Type {
                kind: TypeKind::Unit,
                span,
            }),
            ExprKind::Group(inner) => self.expr(inner, expected, substitutions, locals),
            ExprKind::Array(values) => {
                let element_expected = match expected.map(|ty| &ty.kind) {
                    Some(TypeKind::Array { element, .. }) | Some(TypeKind::Slice(element)) => {
                        Some(&**element)
                    }
                    _ => None,
                };
                let mut first = None;
                for value in values.iter_mut() {
                    let ty = self.expr(
                        value,
                        element_expected.or(first.as_ref()),
                        substitutions,
                        locals,
                    );
                    if first.is_none() {
                        first = ty;
                    }
                }
                expected.cloned().or_else(|| {
                    first.map(|element| Type {
                        span,
                        kind: TypeKind::Array {
                            element: Box::new(element),
                            length: Box::new(Expr {
                                span,
                                kind: ExprKind::Integer(IntegerLiteral {
                                    value: (values.len() as u64).into(),
                                    suffix: None,
                                }),
                            }),
                        },
                    })
                })
            }
            ExprKind::Subscript { value, index } => {
                let ty = self.expr(value, None, substitutions, locals);
                self.expr(index, None, substitutions, locals);
                match ty.map(|ty| ty.kind) {
                    Some(TypeKind::Slice(inner)) | Some(TypeKind::Array { element: inner, .. }) => {
                        Some(*inner)
                    }
                    _ => expected.cloned(),
                }
            }
            ExprKind::Tuple(values) => {
                let expected_fields = match expected.map(|ty| &ty.kind) {
                    Some(TypeKind::Tuple(fields)) => Some(fields),
                    _ => None,
                };
                let mut fields = Vec::new();
                for (index, value) in values.iter_mut().enumerate() {
                    let ty = self.expr(
                        value,
                        expected_fields
                            .and_then(|fields| fields.get(index))
                            .map(|field| &field.ty),
                        substitutions,
                        locals,
                    );
                    if let Some(ty) = ty {
                        fields.push(TypeField {
                            name: None,
                            span: value.span,
                            ty,
                        });
                    }
                }
                (fields.len() == values.len()).then_some(Type {
                    kind: TypeKind::Tuple(fields),
                    span,
                })
            }
            ExprKind::Call { callee, arguments } => {
                if let ExprKind::Name(name) = &mut callee.kind
                    && self.templates.contains_key(&name.text)
                {
                    let name_text = name.text.clone();
                    if let Some(args) =
                        self.infer_arguments(&name_text, arguments, expected, substitutions, locals)
                        && let Some(instance) =
                            self.instantiate(&name_text, args, substitutions, locals, span)
                    {
                        name.text = instance;
                    }
                }
                self.expr(callee, expected, substitutions, locals);
                let signature =
                    expr_path(callee).and_then(|path| self.signature(&path.text(), locals));
                let payload = expr_path(callee).and_then(|path| self.payload(&path, locals));
                for (index, argument) in arguments.iter_mut().enumerate() {
                    let ty = signature
                        .as_ref()
                        .and_then(|(parameters, _)| parameters.get(index))
                        .or_else(|| {
                            payload
                                .as_ref()
                                .and_then(|fields| fields.get(index))
                                .map(|field| &field.ty)
                        });
                    self.expr(argument, ty, substitutions, locals);
                }
                signature.map(|(_, result)| result).or_else(|| {
                    expr_path(callee)
                        .filter(|_| payload.is_some())
                        .map(|path| named_type(&path.segments[0]))
                })
            }
            ExprKind::Struct { path, fields } => {
                self.specialize_path(path, None, expected, substitutions, locals);
                let payload = self.payload(path, locals);
                for field in fields {
                    let name = field.name.as_ref().or(match &field.value.kind {
                        ExprKind::Name(name) => Some(name),
                        _ => None,
                    });
                    let ty = payload
                        .as_ref()
                        .and_then(|fields| {
                            fields.iter().find(|field| {
                                field.name.as_ref().map(|name| &name.text)
                                    == name.map(|name| &name.text)
                            })
                        })
                        .map(|field| &field.ty);
                    self.expr(&mut field.value, ty, substitutions, locals);
                }
                Some(named_type(&path.segments[0]))
            }
            ExprKind::Block(block) | ExprKind::Logic(block) => {
                self.block(block, expected, substitutions, &mut locals.clone())
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expr(condition, None, substitutions, locals);
                let then_ty = self.block(then_branch, expected, substitutions, &mut locals.clone());
                self.expr(
                    else_branch,
                    expected.or(then_ty.as_ref()),
                    substitutions,
                    locals,
                )
                .or(then_ty)
            }
            ExprKind::Match { scrutinee, arms } => {
                let scrutinee_ty = self.expr(scrutinee, None, substitutions, locals);
                let mut result = expected.cloned();
                for arm in arms {
                    let mut locals = locals.clone();
                    self.pattern(
                        &mut arm.pattern,
                        scrutinee_ty.as_ref(),
                        substitutions,
                        &mut locals,
                    );
                    let ty = self.expr(&mut arm.body, result.as_ref(), substitutions, &locals);
                    if result.is_none() {
                        result = ty;
                    }
                }
                result
            }
            ExprKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.expr(constructor, expected, substitutions, locals);
                self.expr(evidence, None, substitutions, locals);
                expected.cloned()
            }
            ExprKind::Form {
                form, arguments, ..
            } => {
                for arg in &mut *arguments {
                    self.expr(arg, None, substitutions, locals);
                }
                match form {
                    Form::Prop => Some(named_type(&Name {
                        text: "Prop".into(),
                        span,
                    })),
                    Form::Prove => arguments.first().map(|claim| Type {
                        kind: TypeKind::Proof(Box::new(claim.clone())),
                        span,
                    }),
                    _ => expected.cloned(),
                }
            }
            ExprKind::Cast {
                expr,
                ty,
                source_hint,
                ..
            } => {
                self.ty(ty, substitutions, locals);
                *source_hint = self.expr(expr, None, substitutions, locals).map(Box::new);
                Some(ty.clone())
            }
            ExprKind::Ref { mutable, expr } => {
                self.expr(expr, None, substitutions, locals)
                    .map(|inner| Type {
                        kind: TypeKind::Ref {
                            lifetime: None,
                            mutable: *mutable,
                            inner: Box::new(inner),
                        },
                        span,
                    })
            }
            ExprKind::Not(inner) | ExprKind::Unary { expr: inner, .. } => {
                self.expr(inner, expected, substitutions, locals)
            }
            ExprKind::Binary {
                operator,
                left,
                right,
                ..
            } => {
                let comparison = matches!(
                    operator,
                    BinaryOp::Equal
                        | BinaryOp::NotEqual
                        | BinaryOp::Less
                        | BinaryOp::LessEqual
                        | BinaryOp::Greater
                        | BinaryOp::GreaterEqual
                );
                let operand_expected = if comparison { None } else { expected };
                let ty = self.expr(left, operand_expected, substitutions, locals);
                self.expr(
                    right,
                    ty.as_ref().or(operand_expected),
                    substitutions,
                    locals,
                );
                if comparison {
                    ty.map(|ty| {
                        named_type(&Name {
                            text: if self.logical(&ty) { "Bool" } else { "bool" }.into(),
                            span,
                        })
                    })
                } else {
                    ty
                }
            }
            ExprKind::Range { lower, upper, .. } => {
                let ty = self.expr(lower, expected, substitutions, locals);
                self.expr(upper, ty.as_ref().or(expected), substitutions, locals);
                ty
            }
            ExprKind::Member { value, name } => {
                let ty = self.expr(value, None, substitutions, locals);
                let path = ty.as_ref().and_then(type_name).map(|name| Path {
                    segments: vec![name.clone()],
                    span,
                });
                path.and_then(|path| self.payload(&path, locals))
                    .and_then(|fields| {
                        fields.into_iter().find(|field| {
                            field
                                .name
                                .as_ref()
                                .is_some_and(|field| field.text == name.text)
                        })
                    })
                    .map(|field| field.ty)
            }
            ExprKind::Index { value, index, .. } => {
                match self
                    .expr(value, None, substitutions, locals)
                    .map(|ty| ty.kind)
                {
                    Some(TypeKind::Tuple(fields)) => index
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| fields.get(index).map(|field| field.ty.clone())),
                    _ => None,
                }
            }
            ExprKind::Closure { parameters, body } => {
                let mut locals = locals.clone();
                for parameter in &mut *parameters {
                    self.ty(&mut parameter.ty, substitutions, &locals);
                    locals.insert(parameter.name.text.clone(), parameter.ty.clone());
                }
                let result = match expected.map(|ty| &ty.kind) {
                    Some(TypeKind::LogicalFunction { result, .. })
                    | Some(TypeKind::Function { result, .. }) => Some(&**result),
                    _ => None,
                };
                let body_type = self.expr(body, result, substitutions, &locals);
                body_type.map(|result| Type {
                    span,
                    kind: TypeKind::LogicalFunction {
                        parameters: parameters
                            .iter()
                            .map(|parameter| TypeField {
                                name: Some(parameter.name.clone()),
                                ty: parameter.ty.clone(),
                                span: parameter.span,
                            })
                            .collect(),
                        result: Box::new(result),
                    },
                })
            }
            ExprKind::Forall { parameters, body } | ExprKind::Exists { parameters, body } => {
                let mut locals = locals.clone();
                for parameter in parameters {
                    self.ty(&mut parameter.ty, substitutions, &locals);
                    locals.insert(parameter.name.text.clone(), parameter.ty.clone());
                }
                self.block(body, None, substitutions, &mut locals);
                None
            }
            ExprKind::Loop { body } => {
                self.block(body, None, substitutions, &mut locals.clone());
                None
            }
            ExprKind::While {
                condition,
                pattern,
                body,
            } => {
                let ty = self.expr(condition, None, substitutions, locals);
                let mut locals = locals.clone();
                if let Some(pattern) = pattern {
                    self.pattern(pattern, ty.as_ref(), substitutions, &mut locals);
                }
                self.block(body, None, substitutions, &mut locals);
                None
            }
            ExprKind::For {
                iterable,
                pattern,
                body,
            } => {
                let ty = self.expr(iterable, None, substitutions, locals);
                let mut locals = locals.clone();
                self.pattern(pattern, ty.as_ref(), substitutions, &mut locals);
                self.block(body, None, substitutions, &mut locals);
                None
            }
            ExprKind::Return(value) => {
                let result = self.return_type.clone();
                value.as_mut().and_then(|value| {
                    self.expr(value, result.as_ref().or(expected), substitutions, locals)
                })
            }
            ExprKind::Break(value) => value
                .as_mut()
                .and_then(|value| self.expr(value, expected, substitutions, locals)),
            ExprKind::GenericApply { .. } => unreachable!("application expanded above"),
            ExprKind::Hole | ExprKind::Error | ExprKind::String(_) | ExprKind::Continue => {
                expected.cloned()
            }
        };
        inferred.or_else(|| expected.cloned())
    }

    fn pattern(
        &mut self,
        pattern: &mut Pattern,
        expected: Option<&Type>,
        substitutions: &Types,
        locals: &mut Types,
    ) {
        match &mut pattern.kind {
            PatternKind::Name { name, .. } => {
                if let Some(ty) = expected {
                    locals.insert(name.text.clone(), ty.clone());
                } else {
                    locals.remove(&name.text);
                }
            }
            PatternKind::Binding { name, pattern, .. } => {
                if let Some(ty) = expected {
                    locals.insert(name.text.clone(), ty.clone());
                }
                self.pattern(pattern, expected, substitutions, locals);
            }
            PatternKind::Group(pattern) => self.pattern(pattern, expected, substitutions, locals),
            PatternKind::Evidence {
                constructor,
                evidence,
                ..
            } => {
                self.pattern(constructor, expected, substitutions, locals);
                self.pattern(evidence, None, substitutions, locals);
            }
            PatternKind::Tuple(patterns) => {
                let fields = match expected.map(|ty| &ty.kind) {
                    Some(TypeKind::Tuple(fields)) => Some(fields),
                    _ => None,
                };
                for (index, pattern) in patterns.iter_mut().enumerate() {
                    self.pattern(
                        pattern,
                        fields
                            .and_then(|fields| fields.get(index))
                            .map(|field| &field.ty),
                        substitutions,
                        locals,
                    );
                }
            }
            PatternKind::Variant { path, arguments } => {
                self.specialize_path(path, None, expected, substitutions, locals);
                let fields = self.payload(path, locals);
                if let Some(arguments) = arguments {
                    for (index, pattern) in arguments.iter_mut().enumerate() {
                        self.pattern(
                            pattern,
                            fields
                                .as_ref()
                                .and_then(|fields| fields.get(index))
                                .map(|field| &field.ty),
                            substitutions,
                            locals,
                        );
                    }
                }
            }
            PatternKind::Struct { path, fields, .. } => {
                self.specialize_path(path, None, expected, substitutions, locals);
                let payload = self.payload(path, locals);
                for field in fields {
                    let name = field.name.as_ref().or(match &field.pattern.kind {
                        PatternKind::Name { name, .. } => Some(name),
                        _ => None,
                    });
                    let ty = payload
                        .as_ref()
                        .and_then(|fields| {
                            fields.iter().find(|field| {
                                field.name.as_ref().map(|name| &name.text)
                                    == name.map(|name| &name.text)
                            })
                        })
                        .map(|field| &field.ty);
                    self.pattern(&mut field.pattern, ty, substitutions, locals);
                }
            }
            PatternKind::Wildcard
            | PatternKind::Unit
            | PatternKind::Bool(_)
            | PatternKind::Integer(_) => {}
        }
    }
}

fn all_parameters(declaration: &Declaration) -> &[GenericParameter] {
    match &declaration.kind {
        DeclarationKind::Function { generics, .. }
        | DeclarationKind::Struct { generics, .. }
        | DeclarationKind::Enum { generics, .. }
        | DeclarationKind::Prop { generics, .. } => generics,
        _ => &[],
    }
}
fn parameters(declaration: &Declaration) -> Vec<GenericParameter> {
    all_parameters(declaration)
        .iter()
        .filter(|parameter| !parameter.lifetime)
        .cloned()
        .collect()
}
fn declaration_name(declaration: &Declaration) -> Option<&Name> {
    match &declaration.kind {
        DeclarationKind::Function { name, .. }
        | DeclarationKind::Struct { name, .. }
        | DeclarationKind::Enum { name, .. }
        | DeclarationKind::Prop { name, .. }
        | DeclarationKind::Constant { name, .. } => Some(name),
        _ => None,
    }
}
fn rename_instance(declaration: &mut Declaration, instance: &str) {
    match &mut declaration.kind {
        DeclarationKind::Function { name, generics, .. }
        | DeclarationKind::Struct { name, generics, .. }
        | DeclarationKind::Enum { name, generics, .. }
        | DeclarationKind::Prop { name, generics, .. } => {
            name.text = instance.to_owned();
            generics.retain(|parameter| parameter.lifetime);
        }
        _ => unreachable!(),
    }
}
fn named_type(name: &Name) -> Type {
    Type {
        kind: TypeKind::Named(name.clone()),
        span: name.span,
    }
}
fn type_name(ty: &Type) -> Option<&Name> {
    match &ty.kind {
        TypeKind::Named(name) => Some(name),
        TypeKind::Path { path, arguments }
            if arguments
                .iter()
                .all(|arg| matches!(arg.kind, TypeKind::Lifetime(_))) =>
        {
            path.single()
        }
        TypeKind::Group(inner) | TypeKind::Ref { inner, .. } => type_name(inner),
        _ => None,
    }
}
fn expr_path(expr: &Expr) -> Option<Path> {
    match &expr.kind {
        ExprKind::Name(name) => Some(Path {
            segments: vec![name.clone()],
            span: expr.span,
        }),
        ExprKind::Path(path) => Some(*path.clone()),
        _ => None,
    }
}
fn derives_logical(declaration: &Declaration) -> bool {
    declaration.attributes.iter().any(|attribute| matches!(&attribute.kind, AttributeKind::Derive(paths) if paths.iter().any(|path| path.text() == "Logical")))
}
fn infer_type_arguments(
    template: &Type,
    actual: &Type,
    parameters: &HashSet<String>,
    inferred: &mut Types,
    origins: &HashMap<String, (String, Vec<Type>)>,
) {
    if let TypeKind::Path { path, arguments } = &actual.kind
        && arguments
            .iter()
            .all(|argument| matches!(argument.kind, TypeKind::Lifetime(_)))
        && origins.contains_key(&path.text())
    {
        let nominal = Type {
            span: actual.span,
            kind: TypeKind::Named(Name {
                text: path.text(),
                span: path.span,
            }),
        };
        infer_type_arguments(template, &nominal, parameters, inferred, origins);
        return;
    }
    match (&template.kind, &actual.kind) {
        (TypeKind::Path { path, arguments }, TypeKind::Named(name))
            if origins.contains_key(&name.text) =>
        {
            let (origin, actual_arguments) = &origins[&name.text];
            if *origin == path.text() {
                for (template, actual) in arguments
                    .iter()
                    .filter(|argument| !matches!(argument.kind, TypeKind::Lifetime(_)))
                    .zip(actual_arguments)
                {
                    infer_type_arguments(template, actual, parameters, inferred, origins);
                }
            }
        }
        (TypeKind::Named(name), _) if parameters.contains(&name.text) => {
            inferred
                .entry(name.text.clone())
                .or_insert_with(|| actual.clone());
        }
        (TypeKind::Group(inner), _) => {
            infer_type_arguments(inner, actual, parameters, inferred, origins)
        }
        (_, TypeKind::Group(inner)) => {
            infer_type_arguments(template, inner, parameters, inferred, origins)
        }
        (TypeKind::Ref { inner: left, .. }, TypeKind::Ref { inner: right, .. }) => {
            infer_type_arguments(left, right, parameters, inferred, origins)
        }
        (
            TypeKind::LogicalFunction {
                parameters: left,
                result: lr,
            },
            TypeKind::LogicalFunction {
                parameters: right,
                result: rr,
            },
        ) => {
            for (left, right) in left.iter().zip(right) {
                infer_type_arguments(&left.ty, &right.ty, parameters, inferred, origins);
            }
            infer_type_arguments(lr, rr, parameters, inferred, origins);
        }
        (TypeKind::Tuple(left), TypeKind::Tuple(right)) => {
            for (left, right) in left.iter().zip(right) {
                infer_type_arguments(&left.ty, &right.ty, parameters, inferred, origins);
            }
        }
        _ => {}
    }
}

/// A stable structural key: Debug is only used as a local serialization; every
/// source Span is discarded outside quoted strings. Distinct source occurrences
/// therefore share an instance; strings containing `Span { ... }` remain intact.
fn type_key(ty: &Type) -> String {
    if let TypeKind::Group(inner) = &ty.kind {
        return type_key(inner);
    }
    let text = format!("{ty:?}");
    let bytes = text.as_bytes();
    let mut output = String::new();
    let (mut index, mut quoted, mut escaped) = (0, false, false);
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if !quoted
            && text[index..].starts_with("Span {")
            && let Some(end) = text[index..].find('}')
        {
            output.push_str("Span");
            index += end + 1;
            continue;
        }
        if ch == '"' && !escaped {
            quoted = !quoted;
        }
        escaped = quoted && ch == '\\' && !escaped;
        // Rust Debug escapes non-ASCII identifiers only when necessary; copy a
        // Unicode scalar rather than slicing in the middle of its UTF-8 bytes.
        let ch = text[index..].chars().next().unwrap();
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

fn has_open_proof(ty: &Type, locals: &Types) -> bool {
    match &ty.kind {
        TypeKind::Proof(expr) => {
            // Conservative until scoped dependent generic instances exist.
            let debug = format!("{expr:?}");
            locals
                .keys()
                .any(|name| debug.contains(&format!("text: {name:?},")))
        }
        TypeKind::Group(inner) | TypeKind::Ref { inner, .. } | TypeKind::Slice(inner) => {
            has_open_proof(inner, locals)
        }
        TypeKind::Array { element, .. } => has_open_proof(element, locals),
        TypeKind::Tuple(fields) => fields.iter().any(|field| has_open_proof(&field.ty, locals)),
        TypeKind::Path { arguments, .. } => arguments.iter().any(|ty| has_open_proof(ty, locals)),
        TypeKind::Function { parameters, result }
        | TypeKind::LogicalFunction { parameters, result } => {
            parameters
                .iter()
                .any(|field| has_open_proof(&field.ty, locals))
                || has_open_proof(result, locals)
        }
        _ => false,
    }
}

/// Quantifier keywords are surface sugar for the ordinary checked prelude.
fn quantifier_expression(
    universal: bool,
    parameters: &[Parameter],
    body: &Block,
    span: Span,
) -> Expr {
    let mut predicate_body = Expr {
        span: body.span,
        kind: ExprKind::Form {
            form: Form::Prop,
            name_span: span,
            arguments: vec![(**body.tail.as_ref().expect("checked quantifier formula")).clone()],
        },
    };
    for parameter in parameters.iter().rev() {
        let name = Name {
            text: if universal { "ForAll" } else { "Exists" }.into(),
            span,
        };
        let callee = Expr {
            span,
            kind: ExprKind::GenericApply {
                callee: Box::new(Expr {
                    span,
                    kind: ExprKind::Name(name),
                }),
                arguments: vec![parameter.ty.clone()],
            },
        };
        let closure = Expr {
            span,
            kind: ExprKind::Closure {
                parameters: vec![parameter.clone()],
                body: Box::new(predicate_body),
            },
        };
        predicate_body = Expr {
            span,
            kind: ExprKind::Call {
                callee: Box::new(callee),
                arguments: vec![closure],
            },
        };
    }
    predicate_body
}

/// Evidence does not store a runtime value of each type named in its claim.
/// Actual aggregate payloads do, and their generic Logical bounds are explicit.
fn stores_parameter(declaration: &Declaration, name: &str) -> bool {
    fn mentions(ty: &Type, name: &str) -> bool {
        match &ty.kind {
            TypeKind::Named(found) => found.text == name,
            TypeKind::Path { arguments, .. } => arguments.iter().any(|ty| mentions(ty, name)),
            TypeKind::Group(inner) | TypeKind::Ref { inner, .. } | TypeKind::Slice(inner) => {
                mentions(inner, name)
            }
            TypeKind::Array { element, .. } => mentions(element, name),
            TypeKind::Tuple(fields) => fields.iter().any(|field| mentions(&field.ty, name)),
            TypeKind::Function { parameters, result }
            | TypeKind::LogicalFunction { parameters, result } => {
                parameters.iter().any(|field| mentions(&field.ty, name)) || mentions(result, name)
            }
            _ => false,
        }
    }
    match &declaration.kind {
        DeclarationKind::Struct { fields, .. } => {
            fields.iter().any(|field| mentions(&field.ty, name))
        }
        DeclarationKind::Enum { variants, .. } => variants
            .iter()
            .any(|variant| variant.fields.iter().any(|field| mentions(&field.ty, name))),
        _ => false,
    }
}

#[cfg(test)]
mod lifetime_tests {
    use super::*;
    #[test]
    fn lifetime_parameters_are_preserved_and_do_not_duplicate_type_instances() {
        let mut sources = SourceMap::default();
        let id = sources.add("lifetimes.lc", "struct Borrowed<'a, T> { item: &'a T } struct View<'a> { slice: &'a [u8] } fn a<'a>(x: Borrowed<'a, u8>) -> () { () } fn b<'b>(x: Borrowed<'b, u8>) -> () { () }");
        let parsed = crate::parser::parse(sources.get(id));
        assert!(parsed.is_success());
        let mut previews = Previews::default();
        for feature in [Feature::LogicalData, Feature::HeapViews] {
            if feature.status() == crate::preview::Status::Preview {
                previews.enable(feature.name()).unwrap();
            }
        }
        let (program, diagnostics, _) = specialize(&parsed.program, &previews);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        let structs: Vec<_> = program
            .declarations
            .iter()
            .filter(|d| matches!(d.kind, DeclarationKind::Struct { .. }))
            .collect();
        assert_eq!(structs.len(), 2);
        for declaration in structs {
            let DeclarationKind::Struct { generics, .. } = &declaration.kind else {
                unreachable!()
            };
            assert_eq!(generics.len(), 1);
            assert!(generics[0].lifetime);
        }
        let names: Vec<_> = program
            .declarations
            .iter()
            .filter_map(|d| {
                if let DeclarationKind::Function { parameters, .. } = &d.kind {
                    match &parameters[0].ty.kind {
                        TypeKind::Path { path, arguments } => {
                            Some((path.text(), arguments.clone()))
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].0, names[1].0);
        assert!(matches!(&names[0].1[0].kind, TypeKind::Lifetime(name) if name.text == "'a"));
        assert!(matches!(&names[1].1[0].kind, TypeKind::Lifetime(name) if name.text == "'b"));
    }
}
