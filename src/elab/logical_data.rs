//! Checked source derivation of logical aggregate types.
use super::env::{Elab, Env};
use crate::ast::{self, AttributeKind};
use crate::kernel::Type;
use crate::preview::Feature;
use crate::typed::Binder;

pub(super) fn has_logical_derive(attributes: &[ast::Attribute]) -> bool {
    attributes.iter().any(|attribute| match &attribute.kind {
        AttributeKind::Derive(paths) => paths
            .iter()
            .any(|path| path.single().is_some_and(|name| name.text == "Logical")),
        _ => false,
    })
}

impl Env<'_> {
    pub(super) fn check_logical_derive(
        &mut self,
        attributes: &[ast::Attribute],
        fields: &[(String, &[Binder])],
    ) -> Elab<()> {
        let mut found = None;
        for attribute in attributes {
            let AttributeKind::Derive(paths) = &attribute.kind else {
                continue;
            };
            for path in paths {
                if path.single().is_none_or(|name| name.text != "Logical") {
                    continue;
                }
                self.require_preview(Feature::LogicalData, "derive(Logical)", path.span)?;
                if found.replace(path.span).is_some() {
                    return self.fail("L0242", "`Logical` is derived twice", path.span);
                }
            }
        }
        let Some(span) = found else { return Ok(()) };
        for (group, fields) in fields {
            for (index, field) in fields.iter().enumerate() {
                let logical = match &field.ty {
                    Type::Bool => field.ghost,
                    _ => self
                        .session
                        .program()
                        .definitions()
                        .is_logical_type(&field.ty),
                };
                if !logical {
                    let name = if field.name == "_" {
                        index.to_string()
                    } else {
                        field.name.clone()
                    };
                    return self.fail(
                        "L0243",
                        format!("cannot derive Logical: field `{group}.{name}` is not Logical"),
                        span,
                    );
                }
            }
        }
        Ok(())
    }
}

fn direct_self(ty: &ast::Type, name: &str) -> bool {
    match &ty.kind {
        ast::TypeKind::Named(found) => found.text == name,
        ast::TypeKind::Group(inner) => direct_self(inner, name),
        _ => false,
    }
}

pub(super) fn contains_self(ty: &ast::Type, name: &str) -> bool {
    match &ty.kind {
        ast::TypeKind::Named(found) => found.text == name,
        ast::TypeKind::Group(inner)
        | ast::TypeKind::Ref { inner, .. }
        | ast::TypeKind::Slice(inner) => contains_self(inner, name),
        ast::TypeKind::Array { element, .. } => contains_self(element, name),
        ast::TypeKind::Path { path, arguments } => {
            path.text() == name || arguments.iter().any(|ty| contains_self(ty, name))
        }
        ast::TypeKind::Tuple(fields) => fields.iter().any(|field| contains_self(&field.ty, name)),
        ast::TypeKind::Function { parameters, result }
        | ast::TypeKind::LogicalFunction { parameters, result } => {
            parameters
                .iter()
                .any(|field| contains_self(&field.ty, name))
                || contains_self(result, name)
        }
        _ => false,
    }
}

impl Env<'_> {
    pub(super) fn logical_enum(
        &mut self,
        declaration: &ast::Declaration,
        _name: &ast::Name,
        _variants: &[ast::Variant],
    ) -> Elab<super::env::Global> {
        self.logical_enum_group(&[declaration])
            .map(|mut group| group.remove(0).1)
    }

    /// Plan all members without introducing temporary kernel declarations.
    /// The session publishes the complete schemas only after its existing
    /// mutual-data checker has accepted every member and field.
    pub(super) fn logical_enum_group(
        &mut self,
        declarations: &[&ast::Declaration],
    ) -> Elab<Vec<(String, super::env::Global)>> {
        use super::env::{EnumInfo, Global, LOGICAL, VariantInfo};
        use crate::kernel::VarId;
        use crate::typed::{EnumItem, VariantItem};
        use std::rc::Rc;
        let names: Vec<_> = declarations
            .iter()
            .map(|declaration| {
                let ast::DeclarationKind::Enum { name, .. } = &declaration.kind else {
                    unreachable!("dependency groups contain only logical enums")
                };
                name
            })
            .collect();
        let mut items = Vec::new();
        let mut recursive_fields = Vec::new();
        for declaration in declarations {
            let ast::DeclarationKind::Enum { name, variants, .. } = &declaration.kind else {
                unreachable!("dependency groups contain only logical enums")
            };
            self.require_preview(Feature::LogicalData, "derive(Logical)", name.span)?;
            let mut variant_items: Vec<VariantItem> = Vec::new();
            let mut member_recursive = Vec::new();
            for variant in variants {
                if variant_items
                    .iter()
                    .any(|earlier| earlier.name == variant.name.text)
                {
                    return self.fail(
                        "L0202",
                        format!("variant `{}` is declared twice", variant.name.text),
                        variant.name.span,
                    );
                }
                self.start_item(&name.text, true, LOGICAL);
                let mut payload: Vec<Binder> = Vec::new();
                let mut recursive = Vec::new();
                for field in &variant.fields {
                    if let Some(field_name) = &field.name
                        && payload
                            .iter()
                            .any(|earlier| earlier.name == field_name.text)
                    {
                        return self.fail(
                            "L0202",
                            format!("`{}` is declared twice", field_name.text),
                            field_name.span,
                        );
                    }
                    if let Some(target) = names
                        .iter()
                        .position(|member| direct_self(&field.ty, &member.text))
                    {
                        recursive.push((payload.len(), target));
                        // Source planning data only: this placeholder is never
                        // declared in a kernel context or global environment.
                        payload.push(Binder {
                            id: VarId::fresh(),
                            name: field
                                .name
                                .as_ref()
                                .map_or("_".into(), |name| name.text.clone()),
                            ty: Type::Int,
                            ghost: true,
                        });
                    } else {
                        if names
                            .iter()
                            .any(|member| contains_self(&field.ty, &member.text))
                        {
                            return self.fail("L0203", "recursive logical types must occur directly as positive enum payload fields", field.ty.span);
                        }
                        let mut fields = self.telescope(
                            std::iter::once((field.name.as_ref(), &field.ty, field.span)),
                            true,
                        )?;
                        payload.push(fields.remove(0));
                    }
                }
                member_recursive.push(recursive);
                variant_items.push(VariantItem {
                    name: variant.name.text.clone(),
                    payload,
                    named: variant.shape == ast::VariantShape::Struct,
                });
            }
            let fields: Vec<_> = variant_items
                .iter()
                .map(|variant| {
                    (
                        format!("{}::{}", name.text, variant.name),
                        variant.payload.as_slice(),
                    )
                })
                .collect();
            let derives = self.derives(&declaration.attributes, &name.text, &fields)?;
            items.push(EnumItem {
                name: name.text.clone(),
                variants: variant_items,
                derives,
            });
            recursive_fields.push(member_recursive);
        }
        let result = self.session.declare_logical_enum_group(items.len(), |ids| {
            for (item, member_fields) in items.iter_mut().zip(recursive_fields) {
                for (variant, fields) in item.variants.iter_mut().zip(member_fields) {
                    for (field, target) in fields {
                        variant.payload[field].ty = Type::Enum(ids[target]);
                    }
                }
            }
            items
        });
        let checked = match result {
            Ok(checked) => checked,
            Err(error) => return self.internal(error, names[0].span),
        };
        Ok(checked
            .into_iter()
            .zip(declarations)
            .map(|((id, item), declaration)| {
                let name = item.name.clone();
                (
                    name.clone(),
                    Global::Enum(Rc::new(EnumInfo {
                        captures: Vec::new(),
                        id,
                        name,
                        variants: item
                            .variants
                            .into_iter()
                            .map(|variant| VariantInfo {
                                name: variant.name,
                                payload: variant.payload,
                                named: variant.named,
                            })
                            .collect(),
                        derives: item.derives,
                        visibility: declaration.visibility.clone(),
                    })),
                )
            })
            .collect())
    }
    pub(super) fn runtime_recursive_enum(
        &mut self,
        declaration: &ast::Declaration,
        name: &ast::Name,
        variants: &[ast::Variant],
    ) -> Elab<super::env::Global> {
        use super::env::{EnumInfo, Global, LOGICAL, VariantInfo};
        use crate::kernel::VarId;
        use crate::typed::{EnumItem, VariantItem};
        use std::rc::Rc;
        self.require_preview(Feature::HeapViews, "recursive runtime enum", name.span)?;
        let mut items: Vec<VariantItem> = Vec::new();
        let mut recursive_fields = Vec::new();
        for variant in variants {
            if items
                .iter()
                .any(|earlier| earlier.name == variant.name.text)
            {
                return self.fail(
                    "L0202",
                    format!("variant `{}` is declared twice", variant.name.text),
                    variant.name.span,
                );
            }
            self.start_item(&name.text, true, LOGICAL);
            let mut payload: Vec<Binder> = Vec::new();
            let mut recursive = Vec::new();
            for field in &variant.fields {
                if let Some(field_name) = &field.name
                    && payload
                        .iter()
                        .any(|earlier| earlier.name == field_name.text)
                {
                    return self.fail(
                        "L0202",
                        format!("`{}` is declared twice", field_name.text),
                        field_name.span,
                    );
                }
                if matches!(&field.ty.kind,ast::TypeKind::Path {path,arguments} if path.single().is_some_and(|n|n.text=="Box") && arguments.len()==1 && direct_self(&arguments[0],&name.text))
                {
                    recursive.push(payload.len());
                    // Only a source planning placeholder. It is never checked,
                    // placed in a kernel context, or published as a definition.
                    payload.push(Binder {
                        id: VarId::fresh(),
                        name: field
                            .name
                            .as_ref()
                            .map_or("_".into(), |name| name.text.clone()),
                        ty: Type::Boxed(Box::new(Type::Int)),
                        ghost: false,
                    });
                } else {
                    if contains_self(&field.ty, &name.text) {
                        return self.fail(
                            "L0203",
                            "runtime recursive fields must be Box<SelfType>",
                            field.ty.span,
                        );
                    }
                    let mut fields = self.telescope(
                        std::iter::once((field.name.as_ref(), &field.ty, field.span)),
                        true,
                    )?;
                    payload.push(fields.remove(0));
                }
            }
            recursive_fields.push(recursive);
            items.push(VariantItem {
                name: variant.name.text.clone(),
                payload,
                named: variant.shape == ast::VariantShape::Struct,
            });
        }
        let groups: Vec<_> = items
            .iter()
            .map(|variant| {
                (
                    format!("{}::{}", name.text, variant.name),
                    variant.payload.as_slice(),
                )
            })
            .collect();
        let derives = self.derives(&declaration.attributes, &name.text, &groups)?;
        let item = EnumItem {
            name: name.text.clone(),
            variants: items,
            derives: derives.clone(),
        };
        let result = self.session.declare_runtime_recursive_enum(|id| {
            let mut item = item;
            for (variant, fields) in item.variants.iter_mut().zip(recursive_fields) {
                for index in fields {
                    variant.payload[index].ty = Type::Boxed(Box::new(Type::Enum(id)));
                }
            }
            item
        });
        let (id, item) = match result {
            Ok(value) => value,
            Err(error) => return self.internal(error, name.span),
        };
        Ok(Global::Enum(Rc::new(EnumInfo {
            captures: Vec::new(),
            id,
            name: name.text.clone(),
            variants: item
                .variants
                .into_iter()
                .map(|variant| VariantInfo {
                    name: variant.name,
                    payload: variant.payload,
                    named: variant.named,
                })
                .collect(),
            derives,
            visibility: declaration.visibility.clone(),
        })))
    }
}

impl Env<'_> {
    pub(super) fn apply_logical_callable(
        &mut self,
        callee: super::exprs::Value,
        arguments: &[ast::Expr],
        span: crate::source::Span,
    ) -> Elab<super::exprs::Value> {
        use super::exprs::Value;
        use crate::kernel::{Mode, Term, infer_term};
        use crate::typed::Expr;
        let Type::Fn(params, result) = &callee.ty else {
            unreachable!()
        };
        if !self.session.program().definitions().is_logical_type(result) {
            return self.fail(
                "L0270",
                "a logical callable cannot return a runtime value",
                span,
            );
        }
        if params.len() != arguments.len() {
            return self.fail(
                "L0208",
                format!(
                    "this logical callable takes {} arguments; {} were supplied",
                    params.len(),
                    arguments.len()
                ),
                span,
            );
        }
        let mut telescope = params.clone();
        telescope.push((**result).clone());
        let telescope = Type::Tuple(telescope);
        let mut terms = Vec::new();
        let mut exprs = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let ty =
                crate::kernel::telescope_entry(&telescope, index, &terms).expect("known argument");
            let ghost = self.session.program().definitions().is_erased_type(&ty)
                || matches!(ty, Type::Bool | Type::Fn(..));
            let value = self.observation_argument(argument, &ty, ghost, None)?;
            terms.push(self.term(&value, argument.span)?);
            exprs.push(value.expr);
        }
        let ty =
            crate::kernel::telescope_entry(&telescope, params.len(), &terms).expect("known result");
        let function = self.term(&callee, span)?;
        let call = Term::call(function.clone(), terms);
        let checked = infer_term(&mut self.ctx, &call, Mode::Logical);
        self.kernel(checked, span)?;
        let apply = Expr::LogicalApply {
            callee: function,
            arguments: exprs,
            ty: ty.clone(),
        };
        // A logical callee value may have been produced by ordinary code.
        // Retain that eager computation before the eager argument effects.
        let expression = if crate::typed::is_pure(&callee.expr) {
            apply
        } else {
            Expr::Block(crate::typed::Block {
                stmts: vec![crate::typed::Stmt::Let {
                    pattern: crate::typed::Pattern::Wildcard,
                    value: callee.expr,
                }],
                tail: Some(Box::new(apply)),
            })
        };
        Ok(Value::new(expression, ty))
    }
}

impl Env<'_> {
    pub(super) fn measured_call(
        &mut self,
        arguments: &[ast::Expr],
        span: crate::source::Span,
    ) -> Elab<super::exprs::Value> {
        use super::exprs::Value;
        use crate::typed::Expr;
        self.require_preview(Feature::LogicalData, "checked measured recursion", span)?;
        if !self.total || self.recursion.is_none() {
            return self.fail(
                "L0203",
                "recurse! is available only inside a recursive logic function",
                span,
            );
        }
        let [evidence, call] = arguments else {
            return self.fail(
                "L0208",
                "recurse! takes descent evidence and a recursive call",
                span,
            );
        };
        let evidence = self.infer(evidence)?;
        if !matches!(evidence.ty, Type::Proof(_)) {
            return self.fail(
                "L0220",
                "recurse! requires proof of a nonnegative decreasing measure",
                span,
            );
        }
        let call = self.infer(call)?;
        let ty = call.ty.clone();
        let tuple = Expr::Tuple {
            ty: Type::Tuple(vec![evidence.ty, ty.clone()]),
            fields: vec![evidence.expr, call.expr],
        };
        Ok(Value::new(
            Expr::Field {
                target: Box::new(tuple),
                index: 1,
                name: None,
                ty: ty.clone(),
            },
            ty,
        ))
    }
}
