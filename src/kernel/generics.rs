//! Generic templates are syntax for checked monomorphic declarations.
//!
//! A parameter is represented by a reserved, undeclared nominal identity
//! only while building the template. It is never a well-formed kernel type.
//! Instantiation substitutes types throughout the template and then calls
//! the ordinary declaration checker; there is no polymorphic proof rule.

use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_PARAMETER: AtomicUsize = AtomicUsize::new(0);

use super::check::check_type;
use super::context::Context;
use super::defs::{Definitions, PropVariant};
use super::depth::check_depth;
use super::error::KernelError;
use super::term::{Depth, EnumId, FnId, PropId, Rebind, StructId, Term, Type, VarId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeBound {
    Any,
    Logical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericId(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenericInstance {
    Struct(StructId),
    Enum(EnumId),
    Function(FnId),
    Proposition(PropId),
}

#[derive(Clone, Debug)]
pub enum GenericDeclaration {
    Struct(Type),
    Enum(Vec<Type>),
    /// The body is under the signature's parameter telescope.
    Function {
        signature: Type,
        body: Term,
    },
    Proposition {
        params: Vec<Type>,
        variants: Vec<PropVariant>,
    },
}

impl GenericDeclaration {
    pub fn function(signature: Type, body: impl FnOnce(&[Term]) -> Term) -> Self {
        let arity = match &signature {
            Type::Fn(params, _) => params.len(),
            _ => 0,
        };
        let vars: Vec<_> = (0..arity).map(|_| VarId::fresh()).collect();
        let arguments: Vec<_> = vars.iter().copied().map(Term::Free).collect();
        let body = body(&arguments).close_over(&vars);
        Self::Function { signature, body }
    }

    fn check_depth(&self) -> Result<(), KernelError> {
        match self {
            Self::Struct(ty) => check_depth([ty.into()]),
            Self::Enum(variants) => check_depth(variants.iter().map(Into::into)),
            Self::Function { signature, body } => check_depth([signature.into(), body.into()]),
            Self::Proposition { params, variants } => {
                check_depth(params.iter().map(Into::into))?;
                for variant in variants {
                    match variant {
                        PropVariant::Params(ty) => check_depth([ty.into()])?,
                        PropVariant::Arm { witnesses, body } => {
                            check_depth([witnesses.into(), body.into()])?
                        }
                        PropVariant::Indexed {
                            payload,
                            conclusion,
                        } => check_depth(
                            std::iter::once(payload.into())
                                .chain(conclusion.iter().map(Into::into)),
                        )?,
                    }
                }
                Ok(())
            }
        }
    }

    fn substitute(&self, replacements: &[(StructId, Type)]) -> Self {
        let op = Rebind::SubstituteTypes(replacements);
        let ty = |ty: &Type| ty.rebind(Depth::default(), op);
        let term = |term: &Term| term.rebind(Depth::default(), op);
        match self {
            Self::Struct(fields) => Self::Struct(ty(fields)),
            Self::Enum(variants) => Self::Enum(variants.iter().map(ty).collect()),
            Self::Function { signature, body } => Self::Function {
                signature: ty(signature),
                body: term(body),
            },
            Self::Proposition { params, variants } => Self::Proposition {
                params: params.iter().map(ty).collect(),
                variants: variants
                    .iter()
                    .map(|variant| match variant {
                        PropVariant::Params(fields) => PropVariant::Params(ty(fields)),
                        PropVariant::Arm { witnesses, body } => PropVariant::Arm {
                            witnesses: ty(witnesses),
                            body: term(body),
                        },
                        PropVariant::Indexed {
                            payload,
                            conclusion,
                        } => PropVariant::Indexed {
                            payload: ty(payload),
                            conclusion: conclusion.iter().map(term).collect(),
                        },
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct GenericTemplate {
    parameters: Vec<(StructId, TypeBound)>,
    declaration: GenericDeclaration,
    instances: Vec<(Vec<Type>, GenericInstance)>,
}

impl Definitions {
    /// Records a template without accepting it as a kernel declaration.
    /// Only instantiated, fully checked monomorphic declarations are usable.
    pub fn declare_generic(
        &mut self,
        bounds: Vec<TypeBound>,
        build: impl FnOnce(&[Type]) -> GenericDeclaration,
    ) -> Result<GenericId, KernelError> {
        // Vec cannot contain a usize::MAX-sized declaration table. These
        // identities therefore remain unknown until substituted away.
        let parameters: Vec<_> = bounds
            .into_iter()
            .map(|bound| {
                let serial = NEXT_PARAMETER.fetch_add(1, Ordering::Relaxed);
                assert!(
                    serial < usize::MAX / 2,
                    "generic parameter identity space exhausted"
                );
                (StructId(usize::MAX - serial), bound)
            })
            .collect();
        let arguments: Vec<_> = parameters.iter().map(|(id, _)| Type::Struct(*id)).collect();
        let declaration = build(&arguments);
        declaration.check_depth()?;
        let id = GenericId(self.generics.len());
        self.generics.push(GenericTemplate {
            parameters,
            declaration,
            instances: Vec::new(),
        });
        Ok(id)
    }

    pub fn instantiate_generic(
        &mut self,
        id: GenericId,
        arguments: &[Type],
    ) -> Result<GenericInstance, KernelError> {
        let template = self.generics.get(id.0).ok_or(KernelError::UnknownGeneric)?;
        if arguments.len() != template.parameters.len() {
            return Err(KernelError::WrongArity {
                expected: template.parameters.len(),
                found: arguments.len(),
            });
        }
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        for (argument, (_, bound)) in arguments.iter().zip(&template.parameters) {
            check_type(&mut ctx, argument)?;
            if *bound == TypeBound::Logical && !self.is_logical_type(argument) {
                return Err(KernelError::NotLogicalType(argument.clone()));
            }
        }
        if let Some((_, instance)) = template
            .instances
            .iter()
            .find(|(types, _)| types == arguments)
        {
            return Ok(*instance);
        }
        let replacements: Vec<_> = template
            .parameters
            .iter()
            .zip(arguments)
            .map(|((id, _), ty)| (*id, ty.clone()))
            .collect();
        let declaration = template.declaration.substitute(&replacements);
        // Substitution may increase depth even though both inputs passed.
        declaration.check_depth()?;
        let instance = match declaration {
            GenericDeclaration::Struct(fields) => {
                GenericInstance::Struct(self.declare_struct(&fields)?)
            }
            GenericDeclaration::Enum(variants) => {
                GenericInstance::Enum(self.declare_enum(&variants)?)
            }
            GenericDeclaration::Function { signature, body } => {
                GenericInstance::Function(self.declare_fn(&signature, |params| {
                    body.instantiate(params.len(), |i| params[i].clone())
                })?)
            }
            GenericDeclaration::Proposition { params, variants } => {
                GenericInstance::Proposition(self.declare_prop(params, variants)?)
            }
        };
        self.generics[id.0]
            .instances
            .push((arguments.to_vec(), instance));
        Ok(instance)
    }
}
