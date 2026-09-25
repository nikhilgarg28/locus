use super::*;

/// Export the physical interface, never a projected version of a logical trait.
/// Each Rust implementation delegates to the exact body checked for that pair.
pub(super) fn interfaces(
    unit: &Checked,
    export: &mut Interface<'_>,
    exports: &[crate::project::resolve::Export],
) -> String {
    use crate::ast::*;
    use std::fmt::Write;
    let registry = &unit.loaded.graph.traits;
    let mut out = String::new();
    let mut seen = BTreeSet::new();
    for root in exports {
        let item = &unit.loaded.graph.items[root.item];
        let Some(definition) = registry.definitions.get(&item.canonical) else {
            continue;
        };
        if matches!(
            definition.kind,
            crate::ast::DeclarationKind::Trait {
                native: Some(_),
                ..
            }
        ) {
            continue;
        }
        if !seen.insert(item.canonical.clone()) {
            continue;
        }
        let DeclarationKind::Trait { members, .. } = &definition.kind else {
            unreachable!()
        };
        let start_errors = export.errors.len();
        let path = root.path.join("::");
        for member in members {
            if member.attributes.iter().any(|a| a.kind.is_promise()) {
                export.error(
                    &path,
                    root.span,
                    member.span,
                    "trait promises cannot cross the Rust implementation boundary",
                );
            }
            match &member.kind {
                DeclarationKind::Function { logical: true, .. }
                | DeclarationKind::AssociatedType { logical: true, .. } => {
                    export.error(
                        &path,
                        root.span,
                        member.span,
                        "a logical trait member cannot be exported",
                    );
                }
                _ => {}
            }
        }
        let header = trait_text(
            &item.canonical,
            members,
            None,
            unit,
            export,
            root.span,
            &path,
        );
        for implementation in &registry.implementations {
            if implementation.interface != item.canonical {
                continue;
            }
            // Check actual bindings too: an unconstrained associated slot may
            // have a logical realization or expose an unexportable aggregate.
            for ty in implementation.associated.values() {
                let _ = trait_type(ty, unit, export, root.span, &path);
            }
            for method in &implementation.members {
                if method.attributes.iter().any(|a| a.kind.is_promise()) {
                    export.error(
                        &path,
                        root.span,
                        method.span,
                        "trait implementation promises cannot be exported",
                    );
                }
                let n = crate::project::resolve::declared_name(&method.kind).unwrap();
                let qualified = format!("{}::{}", implementation.owner, n.text);
                if let Some(f) = export.module.fns.iter().find(|f| f.name == qualified) {
                    for (_, _, ty) in &f.params {
                        export.ty(
                            ty,
                            &format!("{path} -> implementation -> parameter"),
                            root.span,
                        );
                    }
                    export.ty(
                        &f.result,
                        &format!("{path} -> implementation -> result"),
                        root.span,
                    );
                } else {
                    export.error(
                        &path,
                        root.span,
                        method.span,
                        "this implementation contains an erased logical member",
                    );
                }
            }
        }
        if export.errors.len() != start_errors {
            continue;
        }
        let _ = writeln!(out, "\npub trait {} {{\n{header}}}", item.canonical);
        for implementation in &registry.implementations {
            if implementation.interface != item.canonical {
                continue;
            }
            let body = trait_text(
                &item.canonical,
                members,
                Some(implementation),
                unit,
                export,
                root.span,
                &path,
            );
            let _ = writeln!(
                out,
                "impl {} for {} {{\n{body}}}",
                item.canonical, implementation.owner
            );
        }
    }
    for (interface, definition) in &registry.definitions {
        let DeclarationKind::Trait {
            native: Some(foreign),
            members,
            ..
        } = &definition.kind
        else {
            continue;
        };
        let exported = exports
            .iter()
            .any(|e| unit.loaded.graph.items[e.item].canonical == *interface);
        let implementations: Vec<_> = registry
            .implementations
            .iter()
            .filter(|i| i.interface == *interface)
            .collect();
        if !exported && implementations.is_empty() {
            continue;
        }
        let _ = writeln!(
            out,
            "#[allow(unused_imports)]\n{}use {} as {interface};",
            if exported { "pub " } else { "" },
            foreign.path
        );
        for implementation in implementations {
            for ty in implementation.associated.values() {
                let _ = trait_type(ty, unit, export, implementation.span, &foreign.path);
            }
            for method in &implementation.members {
                let name = crate::project::resolve::declared_name(&method.kind).unwrap();
                let qualified = format!("{}::{}", implementation.owner, name.text);
                if let Some(f) = export.module.fns.iter().find(|f| f.name == qualified) {
                    for (_, _, ty) in &f.params {
                        export.ty(ty, &foreign.path, method.span);
                    }
                    export.ty(&f.result, &foreign.path, method.span);
                } else {
                    export.error(
                        &foreign.path,
                        method.span,
                        method.span,
                        "Rust trait implementations require physical members",
                    );
                }
            }
            let body = trait_text(
                interface,
                members,
                Some(implementation),
                unit,
                export,
                implementation.span,
                &foreign.path,
            );
            let _ = writeln!(
                out,
                "impl {} for {} {{\n{body}}}",
                foreign.path, implementation.owner
            );
        }
    }
    out
}
fn trait_text(
    interface: &str,
    members: &[crate::ast::Declaration],
    implementation: Option<&crate::project::traits::Implementation>,
    unit: &Checked,
    export: &mut Interface<'_>,
    at: Span,
    path: &str,
) -> String {
    use crate::ast::*;
    use std::fmt::Write;
    let mut out = String::new();
    for d in members {
        match &d.kind {
            DeclarationKind::AssociatedType { name, .. } => {
                if let Some(i) = implementation {
                    if let Some(t) = i.associated.get(&name.text) {
                        let ty = trait_type(t, unit, export, at, path);
                        let _ = writeln!(out, "type {} = {ty};", name.text);
                    }
                } else {
                    let _ = writeln!(out, "type {};", name.text);
                }
            }
            DeclarationKind::Constant { name, ty, .. } => {
                let ty = trait_type(ty, unit, export, at, path);
                if let Some(i) = implementation {
                    let _ = writeln!(
                        out,
                        "const {}: {ty} = {}::{};",
                        name.text,
                        i.owner,
                        crate::project::traits::lowered(interface, &name.text)
                    );
                } else {
                    let _ = writeln!(out, "const {}: {ty};", name.text);
                }
            }
            DeclarationKind::Function {
                name,
                self_param,
                parameters,
                result,
                ..
            } => {
                let mut params = Vec::new();
                let mut args = Vec::new();
                if let Some(receiver) = self_param {
                    params.push(
                        match receiver.kind {
                            SelfKind::Value => "self",
                            SelfKind::MutValue if implementation.is_some() => "mut self",
                            SelfKind::MutValue => "self",
                            SelfKind::Ref => "&self",
                            SelfKind::RefMut => "&mut self",
                        }
                        .to_string(),
                    );
                    args.push("self".to_string());
                }
                for (index, p) in parameters.iter().enumerate() {
                    let ty = trait_type(&p.ty, unit, export, at, path);
                    params.push(format!("arg{index}: {ty}"));
                    args.push(format!("arg{index}"));
                }
                let result = trait_type(result, unit, export, at, path);
                let signature = format!("fn {}({}) -> {result}", name.text, params.join(", "));
                if let Some(i) = implementation {
                    let _ = writeln!(
                        out,
                        "{signature} {{ {}::{}({}) }}",
                        i.owner,
                        crate::project::traits::lowered(interface, &name.text),
                        args.join(", ")
                    );
                } else {
                    let _ = writeln!(out, "{signature};");
                }
            }
            _ => {}
        }
    }
    out
}
fn trait_type(
    ty: &crate::ast::Type,
    _unit: &Checked,
    export: &mut Interface<'_>,
    at: Span,
    path: &str,
) -> String {
    use crate::ast::TypeKind;
    let named = match &ty.kind {
        TypeKind::Named(n) => Some(n.text.clone()),
        TypeKind::Path { path, arguments } if arguments.is_empty() => Some(path.text()),
        _ => None,
    };
    if let Some(name) = named {
        if name == "Self"
            || name.starts_with("Self::")
            || name == "bool"
            || crate::kernel::MachineInt::from_name(&name).is_some()
        {
            return name;
        }
        let runtime = export
            .module
            .structs
            .iter()
            .find(|s| s.name == name)
            .map(|s| EType::Struct(s.id))
            .or_else(|| {
                export
                    .module
                    .enums
                    .iter()
                    .find(|e| e.name == name)
                    .map(|e| EType::Enum(e.id))
            });
        if let Some(runtime) = runtime {
            export.ty(&runtime, path, at);
            return name;
        }
        export.error(
            path,
            at,
            ty.span,
            "a logical or unsupported type occurs in the trait interface",
        );
        return "()".into();
    }
    match &ty.kind {
        TypeKind::Unit => "()".into(),
        TypeKind::Never => "!".into(),
        TypeKind::Group(t) => trait_type(t, _unit, export, at, path),
        TypeKind::Ref {
            mutable,
            inner,
            lifetime,
        } => format!(
            "&{}{}{}",
            lifetime
                .as_ref()
                .map_or(String::new(), |n| format!("{} ", n.text)),
            if *mutable { "mut " } else { "" },
            trait_type(inner, _unit, export, at, path)
        ),
        TypeKind::Tuple(fields) => format!(
            "({})",
            fields
                .iter()
                .map(|f| format!("{},", trait_type(&f.ty, _unit, export, at, path)))
                .collect::<String>()
        ),
        TypeKind::Proof(_) => {
            export.error(
                path,
                at,
                ty.span,
                "a proof occurs in the trait interface; trait proof results are not projected",
            );
            "()".into()
        }
        _ => {
            export.error(
                path,
                at,
                ty.span,
                "this trait interface type has no supported Rust export yet",
            );
            "()".into()
        }
    }
}
