//! Opaque physical dispatch. No observer equation or proof axiom is introduced.
use super::{ExecError, ExecFnId, Program};
use crate::kernel::{Context, StructId, Type, check_type, same_type};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DynTableId(pub(crate) usize);

#[derive(Clone, Debug)]
pub struct DynMethod {
    pub name: String,
    pub params: Vec<Type>,
    pub result: Type,
}

#[derive(Clone, Debug)]
pub struct DynInterface {
    pub id: StructId,
    pub name: String,
    pub methods: Vec<DynMethod>,
}

#[derive(Clone, Debug)]
pub struct DynTable {
    pub interface: StructId,
    pub concrete: Type,
    pub methods: Vec<ExecFnId>,
}

impl Program {
    pub fn declare_dyn_interface(
        &mut self,
        name: String,
        methods: Vec<DynMethod>,
    ) -> Result<StructId, ExecError> {
        let mut seen = std::collections::HashSet::new();
        let mut ctx = Context::with_definitions(Rc::new(self.definitions().clone()));
        for method in &methods {
            if !seen.insert(&method.name)
                || !method.params.iter().all(physical)
                || !physical(&method.result)
            {
                return Err(ExecError::InvalidDyn(
                    "methods require unique names and physical scalar/tuple signatures",
                ));
            }
            check_type(
                &mut ctx,
                &Type::Fn(method.params.clone(), Box::new(method.result.clone())),
            )?;
        }
        let id = self.definitions_mut().declare_opaque();
        self.dynamics.push(DynInterface { id, name, methods });
        Ok(id)
    }

    pub fn dyn_interface(&self, id: StructId) -> Option<&DynInterface> {
        self.dynamics.iter().find(|d| d.id == id)
    }

    pub fn dyn_table(&self, id: DynTableId) -> Option<&DynTable> {
        self.dyn_tables.get(id.0)
    }

    pub fn declare_dyn_table(&mut self, table: DynTable) -> Result<DynTableId, ExecError> {
        if !matches!(&table.concrete, Type::Struct(_) | Type::Enum(_))
            || self.definitions().is_logical_type(&table.concrete)
        {
            return Err(ExecError::InvalidDyn(
                "receiver must be a physical nominal type",
            ));
        }
        if self.dyn_tables.iter().any(|existing| {
            existing.interface == table.interface && same_type(&existing.concrete, &table.concrete)
        }) {
            return Err(ExecError::InvalidDyn(
                "duplicate implementation for dynamic interface",
            ));
        }
        let interface = self
            .dyn_interface(table.interface)
            .ok_or(ExecError::InvalidDyn("unknown interface"))?;
        if table.methods.len() != interface.methods.len()
            || matches!(table.concrete, Type::Struct(id) if self.definitions().is_opaque(id))
        {
            return Err(ExecError::InvalidDyn("table shape or concrete receiver"));
        }
        let mut ctx = Context::with_definitions(Rc::new(self.definitions().clone()));
        check_type(&mut ctx, &table.concrete)?;
        for (method, callee) in interface.methods.iter().zip(&table.methods) {
            let actual = &self
                .function(*callee)
                .ok_or(ExecError::UnknownFunction)?
                .signature;
            let mut parameters = vec![table.concrete.clone()];
            parameters.extend(method.params.clone());
            if !same_type(
                actual,
                &Type::Fn(parameters, Box::new(method.result.clone())),
            ) {
                return Err(ExecError::InvalidDyn(
                    "implementation signature differs from slot",
                ));
            }
        }
        let id = DynTableId(self.dyn_tables.len());
        self.dyn_tables.push(table);
        Ok(id)
    }
}

fn physical(ty: &Type) -> bool {
    match ty {
        Type::Bool | Type::U8 | Type::Machine(_) => true,
        Type::Tuple(fields) => fields.iter().all(physical),
        _ => false,
    }
}
