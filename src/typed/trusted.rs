//! Explicit foreign contracts. The runtime implementation has already been
//! checked; only the replacement logical contract and promises are trusted.
//! This boundary is recorded in Program and never adds a kernel declaration.
use super::{ErasureLayout, FnItem, FnRef, LowerError, Session};
use crate::{erased, exec};

impl Session {
    pub fn declare_trusted_buffer(
        &mut self,
        item: &FnItem,
        prototype: FnRef,
        promises: exec::Promises,
        result_layout: ErasureLayout,
        reason: String,
        implementation: String,
    ) -> Result<FnRef, LowerError> {
        let FnRef::Exec(backend) = prototype else {
            return Err(exec::ExecError::InvalidBuffer(
                "foreign implementation must be executable",
            )
            .into());
        };
        let original = self
            .erased
            .fns
            .iter()
            .find(|f| f.reference == prototype)
            .cloned()
            .ok_or(exec::ExecError::UnknownFunction)?;
        if item.math
            || item.params.len() != original.params.len()
            || item.passing != original.passing
        {
            return Err(exec::ExecError::InvalidBuffer(
                "foreign signature does not match the native parameters/passing modes",
            )
            .into());
        }
        for (param, (_, _, expected)) in item.params.iter().zip(&original.params) {
            let actual = erased::type_with_layout(
                &param.ty,
                &self.layouts.binding(param.id),
                &erased::erase_type,
            );
            if &actual != expected {
                return Err(exec::ExecError::InvalidBuffer(
                    "foreign parameter changes the native runtime type",
                )
                .into());
            }
        }
        let result = erased::type_with_layout(&item.result, &result_layout, &erased::erase_type);
        if result != original.result {
            return Err(exec::ExecError::InvalidBuffer(
                "foreign result must preserve the native runtime shape, including evidence slots",
            )
            .into());
        }
        let signature = crate::kernel::Type::function_over(
            &item
                .params
                .iter()
                .map(|p| (p.id, p.ty.clone()))
                .collect::<Vec<_>>(),
            &item.exec_result(),
        );
        let id = self.program.declare_trusted_adapter(
            backend,
            signature,
            promises,
            reason,
            implementation,
        )?;
        let reference = FnRef::Exec(id);
        let mut function = original;
        function.reference = reference;
        function.name = item.name.clone();
        // Parameter identities belong to the checked implementation body;
        // surface call metadata uses its own telescope identities.
        function.result = result;
        self.erased.fns.push(function);
        if let Some(lending) = self.lending.get(&backend).cloned() {
            self.lending.insert(id, lending);
        }
        self.layouts.functions.insert(reference, result_layout);
        Ok(reference)
    }
}
