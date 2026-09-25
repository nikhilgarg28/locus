use super::env::{Elab, Env, FnInfo, Global};
use crate::{
    ast,
    exec::Promises,
    imports::{Foreign, model::PhysicalType},
    kernel::Type,
    typed::{Binder, Passing},
};
use std::rc::Rc;
fn ty(t: &PhysicalType) -> Type {
    match t {
        PhysicalType::Scalar(s) if s == "bool" => Type::Bool,
        PhysicalType::Scalar(s) => Type::machine(
            crate::kernel::MachineInt::from_name(s).expect("normalized native scalar"),
        ),
        PhysicalType::Tuple(fields) => Type::Tuple(fields.iter().map(ty).collect()),
    }
}
impl Env<'_> {
    pub(super) fn foreign_function(
        &mut self,
        name: &ast::Name,
        foreign: &Foreign,
        _visibility: Option<ast::Visibility>,
    ) -> Elab<Global> {
        let Some(signature) = &foreign.entity.signature else {
            return self.fail(
                "L0514",
                foreign
                    .entity
                    .unavailable
                    .clone()
                    .unwrap_or_else(|| "unsupported Rust entity".into()),
                name.span,
            );
        };
        self.start_item(&name.text, false, Promises::default());
        let mut params = Vec::new();
        for (index, input) in signature.inputs.iter().enumerate() {
            let binder = Binder::new(&format!("arg{index}"), ty(input));
            self.declare(&binder, false, name.span)?;
            params.push(binder);
        }
        let result = ty(&signature.output);
        let reference =
            match self
                .session
                .declare_native(&name.text, &foreign.path, &params, &result)
            {
                Ok(r) => r,
                Err(e) => return self.fail("L0514", e.to_string(), name.span),
            };
        Ok(Global::Fn(Rc::new(FnInfo {
            origin: Some(name.span),
            logical: false,
            result_logical: false,
            reference,
            name: name.text.clone(),
            passing: vec![Passing::Value; params.len()],
            params,
            result,
            constant: false,
            promises: Promises::default(),
            // The Rust declaration is public. Source import/alias privacy
            // has already been enforced by the module resolver.
            visibility: Some(ast::Visibility {
                scope: ast::VisibilityScope::Public,
                span: name.span,
            }),
            receiver: false,
        })))
    }
}
