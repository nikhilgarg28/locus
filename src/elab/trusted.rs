//! The explicit, audited boundary to Rust-native collection implementations.
use super::env::{Elab, Env, FnInfo, Global};
use crate::{
    ast,
    exec::{BufferStorage, Promises},
    kernel::{BufferOp, Type},
    typed::{Binder, Block, ErasureLayout, FnItem},
};
use std::rc::Rc;

impl Env<'_> {
    // This boundary mirrors the complete source declaration telescope.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn trusted_function(
        &mut self,
        name: &ast::Name,
        parameters: &[ast::Parameter],
        result: &ast::Type,
        promises: Promises,
        visibility: Option<ast::Visibility>,
        reason: &str,
        implementation: &ast::Path,
    ) -> Elab<Global> {
        self.require_preview(
            crate::preview::Feature::HeapViews,
            "trusted Rust declaration",
            name.span,
        )?;
        if reason.trim().is_empty() {
            return self.fail(
                "L0287",
                "a trusted declaration requires a nonempty reason",
                name.span,
            );
        }
        let Some((owner, method)) = implementation.pair() else {
            return self.fail(
                "L0287",
                "a native collection implementation is written Vec::len, Vec::push, or Vec::get",
                implementation.span,
            );
        };
        let op = match (owner.text.as_str(), method.text.as_str()) {
            ("Vec", "len") => BufferOp::Length,
            ("Vec", "push") => BufferOp::Push,
            ("Vec", "get") => BufferOp::Get,
            _ => {
                return self.fail(
                    "L0287",
                    "this Rust implementation is not registered for foreign execution",
                    implementation.span,
                );
            }
        };
        self.start_item(&name.text, false, promises);
        let mut params = Vec::new();
        let mut passing = Vec::new();
        for parameter in parameters {
            if params
                .iter()
                .any(|p: &Binder| p.name == parameter.name.text)
            {
                return self.fail("L0202", "duplicate parameter", parameter.span);
            }
            let (written, mode) = self.parameter_type(parameter)?;
            let mut binder = Binder::new(&parameter.name.text, written.ty);
            binder.ghost = written.ghost;
            self.session
                .register_binding_layout(binder.id, self.written_parameter_layout(&parameter.ty));
            self.declare(&binder, false, parameter.span)?;
            self.declare_passing(binder.id, mode);
            params.push(binder);
            passing.push(mode);
        }
        let Some(first) = params.first() else {
            return self.fail(
                "L0287",
                "a native collection declaration needs its receiver parameter",
                name.span,
            );
        };
        let Type::Buffer(element) = &first.ty else {
            return self.fail("L0287", "the native receiver must be a Vec", name.span);
        };
        let element = (**element).clone();
        let mut layout = self.session.binding_layout(first.id);
        while let ErasureLayout::Borrowed { inner, .. } = layout {
            layout = *inner;
        }
        let ErasureLayout::Buffer {
            storage: BufferStorage::Vector,
            element: element_layout,
        } = layout
        else {
            return self.fail(
                "L0287",
                "a Vec implementation requires Vec storage",
                name.span,
            );
        };
        let prototype = self.native_buffer(
            op,
            BufferStorage::Vector,
            &element,
            *element_layout,
            0,
            implementation.span,
        )?;
        let (exits, result_ty) = self.result_over_exits(result, &params, &passing)?;
        let item = FnItem {
            name: name.text.clone(),
            math: false,
            params: params.clone(),
            passing: passing.clone(),
            exits,
            result: result_ty,
            body: Block {
                stmts: vec![],
                tail: None,
            },
        };
        let signature_result = item.exec_result();
        let declared = self.session.declare_trusted_buffer(
            &item,
            prototype.reference,
            promises,
            self.written_layout(result),
            reason.to_owned(),
            implementation.text(),
        );
        let reference = match declared {
            Ok(id) => id,
            Err(error) => return self.fail("L0287", error.to_string(), name.span),
        };
        Ok(Global::Fn(Rc::new(FnInfo {
            origin: Some(name.span),
            logical: false,
            result_logical: self.logical_spelling(result),
            reference,
            name: name.text.clone(),
            params,
            result: signature_result,
            constant: false,
            promises,
            passing,
            visibility,
            receiver: false,
        })))
    }
}
