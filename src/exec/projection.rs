//! Oracle-only projection of a checked snapshot to its physical representation.
//! This table is never consulted by the kernel or executable checker.
use std::collections::HashMap;

use crate::erased::{EType, Module, RunError, Value};
use crate::kernel::{EnumId, StructId};

#[derive(Clone, Debug)]
pub(crate) struct ValueProjection {
    ty: EType,
    structs: HashMap<StructId, Vec<EType>>,
    enums: HashMap<EnumId, Vec<Vec<EType>>>,
}

impl ValueProjection {
    pub(crate) fn new(ty: EType, module: &Module) -> Self {
        Self {
            ty,
            structs: module
                .structs
                .iter()
                .map(|s| (s.id, s.fields.iter().map(|(_, t)| t.clone()).collect()))
                .collect(),
            enums: module
                .enums
                .iter()
                .map(|e| (e.id, e.variants.iter().map(|v| v.payload.clone()).collect()))
                .collect(),
        }
    }

    pub(crate) fn apply(&self, value: Value) -> Result<Value, RunError> {
        self.project(&self.ty, value)
    }

    fn fields(&self, types: &[EType], values: Vec<Value>) -> Result<Vec<Value>, RunError> {
        if types.len() != values.len() {
            return Err(RunError::Stuck(
                "snapshot projection has the wrong field count".into(),
            ));
        }
        types
            .iter()
            .zip(values)
            .map(|(ty, value)| self.project(ty, value))
            .collect()
    }

    fn project(&self, ty: &EType, value: Value) -> Result<Value, RunError> {
        let bad = || RunError::Stuck("snapshot projection has the wrong physical shape".into());
        Ok(match ty {
            EType::Ghost => Value::Ghost,
            EType::Proved => Value::Proved,
            EType::Ref(_, inner) => self.project(inner, value)?,
            EType::Boxed(inner) => {
                let Value::Tuple(mut fields) = value else {
                    return Err(bad());
                };
                if fields.len() != 1 {
                    return Err(bad());
                }
                Value::Tuple(vec![self.project(inner, fields.remove(0))?])
            }
            EType::Buffer(inner) | EType::Array(inner, _) | EType::Slice(inner) => {
                let Value::Buffer(fields) = value else {
                    return Err(bad());
                };
                Value::Buffer(
                    fields
                        .into_iter()
                        .map(|v| self.project(inner, v))
                        .collect::<Result<_, _>>()?,
                )
            }
            EType::Tuple(types) => {
                let Value::Tuple(fields) = value else {
                    return Err(bad());
                };
                Value::Tuple(self.fields(types, fields)?)
            }
            EType::Struct(id) | EType::StructApplied(id, _) => {
                let Value::Struct(actual, fields) = value else {
                    return Err(bad());
                };
                if actual != *id {
                    return Err(bad());
                }
                Value::Struct(
                    *id,
                    self.fields(self.structs.get(id).ok_or_else(bad)?, fields)?,
                )
            }
            EType::Enum(id) | EType::EnumApplied(id, _) => {
                let Value::Variant(actual, variant, fields) = value else {
                    return Err(bad());
                };
                if actual != *id {
                    return Err(bad());
                }
                let types = self
                    .enums
                    .get(id)
                    .and_then(|variants| variants.get(variant))
                    .ok_or_else(bad)?;
                Value::Variant(*id, variant, self.fields(types, fields)?)
            }
            EType::Bool | EType::Int(_) | EType::Fn(_, _) => value,
        })
    }
}
