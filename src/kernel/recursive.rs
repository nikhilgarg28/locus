//! Strictly positive finite logical data and checked structural recursion.
use std::rc::Rc;

use super::check::{check_telescope, expect_type, same_type};
use super::classical::term_is_classical;
use super::context::{Context, Mode};
use super::defs::{Definitions, EnumDecl, FnDecl};
use super::depth::{Node, check_depth, push_children};
use super::error::KernelError;
use super::term::{EnumId, FnId, HypId, Proof, Term, Type, VarId, field_type};

impl Definitions {
    /// Declare a finite logical enum group atomically. Each inner Vec is one
    /// member's variants, each variant a tuple telescope.
    pub fn declare_logical_enum_group(
        &mut self,
        count: usize,
        build: impl FnOnce(&[EnumId]) -> Vec<Vec<Type>>,
    ) -> Result<Vec<EnumId>, KernelError> {
        if count == 0 {
            return Err(KernelError::InvalidRecursion(
                "an enum group cannot be empty",
            ));
        }
        let ids: Vec<_> = (0..count).map(|i| EnumId(self.enums.len() + i)).collect();
        let groups = build(&ids);
        if groups.len() != count {
            return Err(KernelError::FieldCount {
                expected: count,
                found: groups.len(),
            });
        }
        let mut checked = Vec::new();
        for variants in groups {
            let mut payloads = Vec::new();
            for variant in variants {
                check_depth([(&variant).into()])?;
                let Type::Tuple(fields) = variant else {
                    return Err(KernelError::NotAProduct(variant));
                };
                payloads.push(fields);
            }
            checked.push(payloads);
        }
        // Publish the complete candidate schemas only inside this transaction.
        // An empty placeholder would allow a dependent field to eliminate a
        // recursive value with zero arms before its real constructors exist.
        let mut candidate = self.clone();
        for variants in &checked {
            candidate.enums.push(EnumDecl {
                parameters: Vec::new(),
                variants: variants.clone(),
                logical: true,
                group: ids.clone(),
            });
        }
        for variants in &checked {
            for fields in variants {
                for field in fields {
                    if !matches!(field, Type::Enum(id) if ids.contains(id))
                        && mentions_group(field.into(), &ids)
                    {
                        return Err(KernelError::InvalidRecursion(
                            "recursive types must occur directly as positive payload fields",
                        ));
                    }
                    if !candidate.is_logical_type(field) {
                        return Err(KernelError::NotLogicalType(field.clone()));
                    }
                }
                let mut ctx = Context::with_definitions(Rc::new(candidate.clone()));
                check_telescope(&mut ctx, fields)?;
            }
        }
        *self = candidate;
        Ok(ids)
    }

    /// Physical finite enums: every recursive occurrence crosses a Box.
    /// Complete schemas are checked atomically; no placeholder enters a context.
    pub fn declare_runtime_enum_group(
        &mut self,
        count: usize,
        build: impl FnOnce(&[EnumId]) -> Vec<Vec<Type>>,
    ) -> Result<Vec<EnumId>, KernelError> {
        if count == 0 {
            return Err(KernelError::InvalidRecursion("empty runtime enum group"));
        }
        let ids: Vec<_> = (0..count).map(|i| EnumId(self.enums.len() + i)).collect();
        let groups = build(&ids);
        if groups.len() != count {
            return Err(KernelError::FieldCount {
                expected: count,
                found: groups.len(),
            });
        }
        let mut candidate = self.clone();
        for variants in groups {
            let mut checked = Vec::new();
            for variant in variants {
                check_depth([(&variant).into()])?;
                let Type::Tuple(fields) = variant else {
                    return Err(KernelError::NotAProduct(variant));
                };
                for field in &fields {
                    if mentions_group(field.into(), &ids)
                        && !matches!(field,Type::Boxed(inner) if matches!(&**inner,Type::Enum(id) if ids.contains(id)))
                    {
                        return Err(KernelError::InvalidRecursion(
                            "runtime recursion must be a direct Box of an enum in the group",
                        ));
                    }
                }
                checked.push(fields);
            }
            candidate.enums.push(EnumDecl {
                parameters: Vec::new(),
                variants: checked,
                logical: false,
                group: ids.clone(),
            });
        }
        for id in &ids {
            for fields in &candidate.enums[id.0].variants {
                let mut ctx = Context::with_definitions(Rc::new(candidate.clone()));
                check_telescope(&mut ctx, fields)?;
            }
        }
        *self = candidate;
        Ok(ids)
    }
    /// Finite declaration family usable by structural logical observations.
    pub fn finite_enum_group(&self, id: EnumId) -> Option<Vec<EnumId>> {
        let decl = self.enums.get(id.0)?;
        if decl.logical || !decl.group.is_empty() {
            Some(if decl.group.is_empty() {
                vec![id]
            } else {
                decl.group.clone()
            })
        } else {
            None
        }
    }

    /// The complete induction group, or the single member for a logical
    /// nonrecursive enum that was registered by derive(Logical).
    pub fn logical_enum_group(&self, id: EnumId) -> Option<Vec<EnumId>> {
        let decl = self.enums.get(id.0)?;
        if !decl.logical {
            return None;
        }
        Some(if decl.group.is_empty() {
            vec![id]
        } else {
            decl.group.clone()
        })
    }

    pub fn is_inductive_prop(&self, id: super::term::PropId) -> bool {
        self.prop(id)
            .is_some_and(|declaration| declaration.inductive)
    }

    pub fn declare_structural_fn(
        &mut self,
        signature: &Type,
        decreasing: usize,
        body: impl FnOnce(FnId, &[Term]) -> Term,
    ) -> Result<FnId, KernelError> {
        check_depth([signature.into()])?;
        let Type::Fn(params, result) = signature else {
            return Err(KernelError::NotAFunction(signature.clone()));
        };
        let decreasing_type = params.get(decreasing).ok_or(KernelError::InvalidRecursion(
            "missing decreasing parameter",
        ))?;
        match decreasing_type {
            Type::Enum(id) if self.finite_enum_group(*id).is_some() => {}
            Type::Proof(claim) if matches!(&**claim, Term::PropApp(id, _) if self.is_inductive_prop(*id)) =>
                {}
            _ => {
                return Err(KernelError::InvalidRecursion(
                    "the decreasing parameter must be logical data or evidence of an inductive predicate",
                ));
            }
        }
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let mut telescope = params.clone();
        telescope.push((**result).clone());
        check_telescope(&mut ctx, &telescope)?;
        let id = FnId(self.fns.len());
        let vars: Vec<_> = (0..params.len()).map(|_| VarId::fresh()).collect();
        let arguments: Vec<_> = vars.iter().copied().map(Term::Free).collect();
        let value = body(id, &arguments);
        check_depth([(&value).into()])?;
        let mut candidate = self.clone();
        candidate.fns.push(FnDecl {
            params: params.clone(),
            result: (**result).clone(),
            body: value.close_over(&vars),
            classical: false,
            executable: false,
        });
        let mut ctx = Context::with_definitions(Rc::new(candidate.clone()));
        for (index, var) in vars.iter().enumerate() {
            let ty = field_type(&telescope, index, |j| arguments[j].clone());
            ctx.declare_with(*var, ty, true)?;
        }
        let expected = field_type(&telescope, params.len(), |j| arguments[j].clone());
        expect_type(&mut ctx, &value, &expected, Mode::Logical)?;
        match decreasing_type {
            Type::Enum(member) => {
                let origins = vec![(Term::Free(vars[decreasing]), *member, false)];
                validate_descent(&candidate, id, decreasing, (&value).into(), &origins)?;
            }
            Type::Proof(_) => {
                let Type::Proof(claim) =
                    field_type(&telescope, decreasing, |j| arguments[j].clone())
                else {
                    unreachable!()
                };
                let Term::PropApp(root, _) = &*claim else {
                    unreachable!()
                };
                let origins = vec![(vars[decreasing], (*claim).clone(), false)];
                validate_proof_descent(
                    &candidate,
                    id,
                    decreasing,
                    *root,
                    (&value).into(),
                    &origins,
                )?;
            }
            _ => unreachable!(),
        }
        candidate.fns[id.0].classical = term_is_classical(&candidate, &value);
        *self = candidate;
        Ok(id)
    }
}

fn mentions_group(root: Node<'_>, group: &[EnumId]) -> bool {
    let mut work = vec![root];
    while let Some(node) = work.pop() {
        if matches!(node, Node::Type(Type::Enum(id)) | Node::Term(Term::Variant(id, _, _)) if group.contains(id))
        {
            return true;
        }
        push_children(node, &mut work);
    }
    false
}

/// (variable, enum member, proper descendant). Freshly opened payload
/// binders preserve identity even below unrelated term/proof binders.
fn validate_descent(
    defs: &Definitions,
    recursive: FnId,
    decreasing: usize,
    node: Node<'_>,
    origins: &[(Term, EnumId, bool)],
) -> Result<(), KernelError> {
    let origin = |term: &Term| {
        origins
            .iter()
            .find(|(value, _, _)| super::check::same(value, term))
            .cloned()
    };
    if let Node::Term(Term::Call(callee, arguments)) = node
        && **callee == Term::Fn(recursive)
    {
        if !arguments
            .get(decreasing)
            .and_then(origin)
            .is_some_and(|(_, _, strict)| strict)
        {
            return Err(KernelError::InvalidRecursion(
                "recursive call does not use a matched proper descendant",
            ));
        }
        for argument in arguments {
            validate_descent(defs, recursive, decreasing, argument.into(), origins)?;
        }
        return Ok(());
    }
    if matches!(node, Node::Term(Term::Fn(id)) if *id == recursive) {
        return Err(KernelError::InvalidRecursion(
            "a recursive function cannot escape as a value",
        ));
    }
    let split = match node {
        Node::Term(Term::Case { scrutinee, .. }) => origin(scrutinee),
        Node::Proof(Proof::CaseData { scrutinee, .. }) => origin(scrutinee),
        _ => None,
    };
    if let Some((_, member, _)) = split {
        let variants = defs.enum_variants(member).ok_or(KernelError::UnknownEnum)?;
        let group = defs.finite_enum_group(member).unwrap();
        for (index, payload) in variants.iter().enumerate() {
            let vars: Vec<_> = payload.iter().map(|_| VarId::fresh()).collect();
            let mut nested = origins.to_vec();
            for (var, ty) in vars.iter().zip(payload) {
                if let Type::Enum(child) = ty
                    && group.contains(child)
                {
                    nested.push((Term::Free(*var), *child, true));
                }
                if let Type::Boxed(inner) = ty
                    && let Type::Enum(child) = &**inner
                    && group.contains(child)
                {
                    nested.push((Term::proj(Term::Free(*var), 0), *child, true));
                }
            }
            match node {
                Node::Term(Term::Case { arms, .. }) => {
                    let body = arms[index]
                        .body
                        .instantiate(vars.len(), |i| Term::Free(vars[i]));
                    validate_descent(defs, recursive, decreasing, (&body).into(), &nested)?;
                }
                Node::Proof(Proof::CaseData { arms, .. }) => {
                    let hyps: Vec<_> = (0..arms[index].hyps).map(|_| HypId::fresh()).collect();
                    let body = arms[index].body.open_arm(&vars, &hyps);
                    validate_descent(defs, recursive, decreasing, (&body).into(), &nested)?;
                }
                _ => unreachable!(),
            }
        }
        // Also scan the result type/goal and scrutinee: recursion hidden
        // there is subject to the same restriction.
        match node {
            Node::Term(Term::Case {
                result, scrutinee, ..
            }) => {
                validate_descent(defs, recursive, decreasing, result.into(), origins)?;
                validate_descent(defs, recursive, decreasing, (&**scrutinee).into(), origins)?;
            }
            Node::Proof(Proof::CaseData {
                goal, scrutinee, ..
            }) => {
                validate_descent(defs, recursive, decreasing, goal.into(), origins)?;
                validate_descent(defs, recursive, decreasing, scrutinee.into(), origins)?;
            }
            _ => unreachable!(),
        }
        return Ok(());
    }
    let mut children = Vec::new();
    push_children(node, &mut children);
    for child in children {
        validate_descent(defs, recursive, decreasing, child, origins)?;
    }
    Ok(())
}

fn proof_variable(term: &Term) -> Option<VarId> {
    match term {
        Term::Free(id) => Some(*id),
        Term::Proof(proof) => match &**proof {
            Proof::OfTerm(term) => proof_variable(term),
            _ => None,
        },
        _ => None,
    }
}

fn validate_proof_descent(
    defs: &Definitions,
    recursive: FnId,
    decreasing: usize,
    predicate: super::term::PropId,
    node: Node<'_>,
    origins: &[(VarId, Term, bool)],
) -> Result<(), KernelError> {
    let origin = |term: &Term| {
        proof_variable(term).and_then(|variable| origins.iter().find(|(id, _, _)| *id == variable))
    };
    if let Node::Term(Term::Call(callee, arguments)) = node
        && **callee == Term::Fn(recursive)
    {
        if !arguments
            .get(decreasing)
            .and_then(origin)
            .is_some_and(|(_, claim, strict)| {
                *strict && matches!(claim, Term::PropApp(id, _) if *id == predicate)
            })
        {
            return Err(KernelError::InvalidRecursion(
                "recursive proof call does not use matched descendant evidence",
            ));
        }
        for argument in arguments {
            validate_proof_descent(
                defs,
                recursive,
                decreasing,
                predicate,
                argument.into(),
                origins,
            )?;
        }
        return Ok(());
    }
    if matches!(node, Node::Term(Term::Fn(id)) if *id == recursive) {
        return Err(KernelError::InvalidRecursion(
            "a recursive function cannot escape as a value",
        ));
    }
    if let Node::Proof(Proof::CaseProof {
        scrutinee,
        goal,
        arms,
    }) = node
        && let Proof::OfTerm(term) = &**scrutinee
        && let Some((_, Term::PropApp(id, arguments), _)) = origin(term)
    {
        let declaration = defs.prop(*id).ok_or(KernelError::UnknownProp)?;
        for (variant, arm) in declaration.variants.iter().zip(arms) {
            let vars: Vec<_> = (0..arm.vars).map(|_| VarId::fresh()).collect();
            let hyps: Vec<_> = (0..arm.hyps).map(|_| HypId::fresh()).collect();
            let skip = if variant.with_params {
                declaration.params.len()
            } else {
                0
            };
            let mut nested = origins.to_vec();
            for (index, var) in vars.iter().enumerate() {
                let ty = field_type(&variant.telescope, skip + index, |j| {
                    if j < skip {
                        arguments[j].clone()
                    } else {
                        Term::Free(vars[j - skip])
                    }
                });
                if let Type::Proof(claim) = ty {
                    nested.push((*var, *claim, true));
                }
            }
            let body = arm.body.open_arm(&vars, &hyps);
            validate_proof_descent(
                defs,
                recursive,
                decreasing,
                predicate,
                (&body).into(),
                &nested,
            )?;
        }
        validate_proof_descent(
            defs,
            recursive,
            decreasing,
            predicate,
            (&**scrutinee).into(),
            origins,
        )?;
        validate_proof_descent(defs, recursive, decreasing, predicate, goal.into(), origins)?;
        return Ok(());
    }
    let mut children = Vec::new();
    push_children(node, &mut children);
    for child in children {
        validate_proof_descent(defs, recursive, decreasing, predicate, child, origins)?;
    }
    Ok(())
}

impl Definitions {
    /// Check and declare one strictly positive recursive named predicate.
    /// Mutual predicate groups remain unsupported; enum groups are separate.
    pub fn declare_inductive_prop(
        &mut self,
        params: Vec<Type>,
        build: impl FnOnce(super::term::PropId) -> Vec<super::defs::PropVariant>,
    ) -> Result<super::term::PropId, KernelError> {
        use super::defs::{PropDecl, PropVariant};
        use super::term::PropId;
        let id = PropId(self.props.len());
        let variants = build(id);
        let mut candidate = self.clone();
        candidate.props.push(PropDecl {
            params: params.clone(),
            variants: Vec::new(),
            inductive: true,
        });
        for param in &params {
            check_depth([param.into()])?;
        }
        for variant in &variants {
            let PropVariant::Arm { witnesses, body } = variant else {
                return Err(KernelError::InvalidRecursion(
                    "inductive predicates require named arms with one body evidence slot",
                ));
            };
            check_depth([witnesses.into(), body.into()])?;
            if mentions_predicate(witnesses.into(), id) {
                return Err(KernelError::InvalidRecursion(
                    "recursive predicates cannot occur in witness types",
                ));
            }
            positive_body(&candidate, id, body)?;
        }
        // Reuse the ordinary declaration checks against the temporary name,
        // then move the checked declaration into the reserved position.
        let checked = candidate.declare_prop(params, variants)?;
        let mut declaration = candidate.props.pop().expect("just declared");
        debug_assert_eq!(checked.0, id.0 + 1);
        declaration.inductive = true;
        candidate.props[id.0] = declaration;
        *self = candidate;
        Ok(id)
    }
}

fn mentions_predicate(root: Node<'_>, id: super::term::PropId) -> bool {
    let mut work = vec![root];
    while let Some(node) = work.pop() {
        if matches!(node, Node::Term(Term::PropApp(found, _)) if *found == id) {
            return true;
        }
        push_children(node, &mut work);
    }
    false
}

fn positive_body(
    defs: &Definitions,
    recursive: super::term::PropId,
    body: &Term,
) -> Result<(), KernelError> {
    if !mentions_predicate(body.into(), recursive) {
        return Ok(());
    }
    let prelude = defs.prelude().ok_or(KernelError::NoPrelude)?;
    match body {
        Term::PropApp(id, arguments) if *id == recursive => {
            if arguments
                .iter()
                .any(|arg| mentions_predicate(arg.into(), recursive))
            {
                return Err(KernelError::InvalidRecursion(
                    "recursive occurrence inside predicate arguments",
                ));
            }
            Ok(())
        }
        Term::PropApp(id, arguments) if *id == prelude.and || *id == prelude.or => {
            for argument in arguments {
                positive_body(defs, recursive, argument)?;
            }
            Ok(())
        }
        Term::PropApp(id, arguments) if defs.quantifier_element(*id).is_some() => {
            let [predicate] = arguments.as_slice() else {
                return Err(KernelError::InvalidRecursion(
                    "one quantifier predicate is required",
                ));
            };
            let Some((params, result, body)) = visible_predicate_lambda(predicate) else {
                return Err(KernelError::InvalidRecursion(
                    "a positive library quantifier needs a visible predicate lambda",
                ));
            };
            if params.len() != 1
                || !same_type(&params[0], defs.quantifier_element(*id).unwrap())
                || *result != Type::Prop
                || params
                    .iter()
                    .any(|ty| mentions_predicate(ty.into(), recursive))
            {
                return Err(KernelError::InvalidRecursion(
                    "a quantifier's element type cannot contain its recursive predicate",
                ));
            }
            positive_body(defs, recursive, body)
        }
        Term::Implies(premise, conclusion)
            if !mentions_predicate((&**premise).into(), recursive) =>
        {
            positive_body(defs, recursive, conclusion)
        }
        Term::Forall(ty, body) | Term::Exists(ty, body)
            if !mentions_predicate(ty.into(), recursive) =>
        {
            positive_body(defs, recursive, body)
        }
        _ => Err(KernelError::InvalidRecursion(
            "recursive predicate must occur visibly and strictly positively; helpers are not assumed admissible",
        )),
    }
}

/// Replace positive P(args) by P(args) and motive(args), keeping choices and
/// binders aligned. Inputs were already checked by positive_body.
pub(super) fn strengthen_body(
    defs: &Definitions,
    recursive: super::term::PropId,
    body: &Term,
    motive: &super::term::TermArm,
) -> Term {
    if !mentions_predicate(body.into(), recursive) {
        return body.clone();
    }
    let prelude = defs
        .prelude()
        .expect("positive recursive body has a prelude");
    match body {
        Term::PropApp(id, arguments) if *id == recursive => prelude.and_prop(
            body.clone(),
            motive
                .body
                .instantiate(arguments.len(), |i| arguments[i].clone()),
        ),
        Term::PropApp(id, arguments) if *id == prelude.and || *id == prelude.or => Term::PropApp(
            *id,
            arguments
                .iter()
                .map(|arg| strengthen_body(defs, recursive, arg, motive))
                .collect(),
        ),
        Term::PropApp(id, arguments) if defs.quantifier_element(*id).is_some() => Term::PropApp(
            *id,
            vec![strengthen_predicate_lambda(
                defs,
                recursive,
                &arguments[0],
                motive,
            )],
        ),
        Term::Implies(premise, conclusion) => Term::implies(
            (**premise).clone(),
            strengthen_body(defs, recursive, conclusion, motive),
        ),
        Term::Forall(ty, inner) | Term::Exists(ty, inner) => {
            let fresh = VarId::fresh();
            let opened = inner.open(&Term::Free(fresh));
            let strengthened = strengthen_body(defs, recursive, &opened, motive).close(fresh);
            if matches!(body, Term::Forall(..)) {
                Term::Forall(ty.clone(), Box::new(strengthened))
            } else {
                Term::Exists(ty.clone(), Box::new(strengthened))
            }
        }
        _ => unreachable!("only accepted positive bodies are strengthened"),
    }
}

// The surface closure keeps a zero-argument lambda wrapper to mark its logical
// mode. This transparent beta redex cannot hide an unknown helper function.
fn visible_predicate_lambda(term: &Term) -> Option<(&[Type], &Type, &Term)> {
    match term {
        Term::Lambda {
            params,
            result,
            body,
        } => Some((params, result, body)),
        Term::Call(callee, arguments) if arguments.is_empty() => match &**callee {
            Term::Lambda { params, body, .. } if params.is_empty() => {
                visible_predicate_lambda(body)
            }
            _ => None,
        },
        _ => None,
    }
}

fn strengthen_predicate_lambda(
    defs: &Definitions,
    recursive: super::term::PropId,
    predicate: &Term,
    motive: &super::term::TermArm,
) -> Term {
    match predicate {
        Term::Lambda {
            params,
            result,
            body,
        } => {
            let fresh = VarId::fresh();
            let opened = body.open(&Term::Free(fresh));
            let strengthened = strengthen_body(defs, recursive, &opened, motive).close(fresh);
            Term::Lambda {
                params: params.clone(),
                result: result.clone(),
                body: Box::new(strengthened),
            }
        }
        Term::Call(callee, arguments) if arguments.is_empty() => match &**callee {
            Term::Lambda {
                params,
                result,
                body,
            } if params.is_empty() => Term::call(
                Term::Lambda {
                    params: vec![],
                    result: result.clone(),
                    body: Box::new(strengthen_predicate_lambda(defs, recursive, body, motive)),
                },
                vec![],
            ),
            _ => unreachable!("checked visible predicate"),
        },
        _ => unreachable!("checked visible predicate"),
    }
}
