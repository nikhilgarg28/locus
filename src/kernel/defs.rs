//! Global declarations: structs, enums, propositions, and math functions.
//!
//! A declaration is checked against the declarations that precede it, so a
//! struct cannot mention itself and a function cannot call itself, directly or
//! through other declarations. With no loops in kernel terms, that makes
//! every declared function total.

use std::rc::Rc;

use super::check::{check_telescope, expect_type, same_types, type_ok};
use super::classical::term_is_classical;
use super::context::{Context, Mode};
use super::depth::check_depth;
use super::error::KernelError;
use super::term::{EnumId, FnId, PropId, StructId, Term, Type, VarId, field_type};

#[derive(Clone, Debug)]
pub(super) struct StructDecl {
    pub(super) parameters: Vec<Type>,
    /// A telescope, as in `Type::Tuple`.
    pub(super) fields: Vec<Type>,
    pub(super) logical: bool,
}

#[derive(Clone, Debug)]
pub(super) struct EnumDecl {
    pub(super) parameters: Vec<Type>,
    /// One payload telescope per variant.
    pub(super) variants: Vec<Vec<Type>>,
    pub(super) logical: bool,
    pub(super) group: Vec<EnumId>,
}

/// A variant of a declared proposition, as given to `declare_prop`.
#[derive(Clone, Debug)]
pub enum PropVariant {
    /// Header parameters followed by witnesses, and one computed body.
    /// The body is under all telescope binders; the kernel adds its sole
    /// evidence parameter itself. Proof-typed witnesses are rejected.
    Arm { witnesses: Type, body: Term },
    /// No stated conclusion: the variant proves the proposition at its
    /// parameters. The telescope lists the parameters first and then the
    /// payload, so payload types may mention the parameters.
    Params(Type),
    /// A stated conclusion: the variant proves the proposition at exactly
    /// these arguments, which are under the payload telescope. The
    /// parameters are not in scope.
    Indexed {
        payload: Type,
        conclusion: Vec<Term>,
    },
}

impl PropVariant {
    /// Build a named arm over a telescope containing the header parameters
    /// first and the witnesses afterwards. The body receives all of them.
    pub fn arm(witnesses: Type, body: impl FnOnce(&[Term]) -> Term) -> Self {
        let arity = match &witnesses {
            Type::Tuple(fields) => fields.len(),
            _ => 0,
        };
        let vars: Vec<_> = (0..arity).map(|_| VarId::fresh()).collect();
        let values: Vec<_> = vars.iter().copied().map(Term::Free).collect();
        Self::Arm {
            witnesses,
            body: body(&values).close_over(&vars),
        }
    }

    /// Builds an indexed variant; `conclusion(payload)` receives the payload
    /// fields as terms.
    pub fn indexed(payload: Type, conclusion: impl FnOnce(&[Term]) -> Vec<Term>) -> Self {
        let arity = match &payload {
            Type::Tuple(fields) => fields.len(),
            _ => 0,
        };
        let vars: Vec<VarId> = (0..arity).map(|_| VarId::fresh()).collect();
        let terms: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
        let conclusion = conclusion(&terms)
            .iter()
            .map(|term| term.close_over(&vars))
            .collect();
        Self::Indexed {
            payload,
            conclusion,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct PropVariantDecl {
    pub(super) with_params: bool,
    /// Parameters then payload when `with_params`; payload only otherwise.
    pub(super) telescope: Vec<Type>,
    /// Under the payload. Empty when `with_params`.
    pub(super) conclusion: Vec<Term>,
}

#[derive(Clone, Debug)]
pub(super) struct PropDecl {
    pub(super) params: Vec<Type>,
    pub(super) variants: Vec<PropVariantDecl>,
    pub(super) inductive: bool,
}

/// The propositions the kernel itself refers to. Excluded middle is stated
/// with `or` and `falsehood`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prelude {
    pub truth: PropId,
    pub falsehood: PropId,
    pub and: PropId,
    pub or: PropId,
}

impl Prelude {
    pub fn truth_prop(&self) -> Term {
        Term::PropApp(self.truth, Vec::new())
    }

    pub fn falsehood_prop(&self) -> Term {
        Term::PropApp(self.falsehood, Vec::new())
    }

    pub fn and_prop(&self, left: Term, right: Term) -> Term {
        Term::PropApp(self.and, vec![left, right])
    }

    pub fn or_prop(&self, left: Term, right: Term) -> Term {
        Term::PropApp(self.or, vec![left, right])
    }

    /// `!p` abbreviates `p => False`.
    pub fn not_prop(&self, prop: Term) -> Term {
        Term::implies(prop, self.falsehood_prop())
    }
}

#[derive(Clone, Debug)]
pub(super) struct FnDecl {
    /// A telescope, as in `Type::Fn`.
    pub(super) params: Vec<Type>,
    /// Under all the parameters.
    pub(super) result: Type,
    /// Under all the parameters.
    pub(super) body: Term,
    /// Whether the body uses excluded middle, directly or through a call.
    pub(super) classical: bool,
    /// Whether the function has a runtime form: its body is an executable
    /// term when its ghost-typed parameters are ghost.
    pub(super) executable: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Definitions {
    pointer_width: super::PointerWidth,
    structs: Vec<StructDecl>,
    pub(super) enums: Vec<EnumDecl>,
    pub(super) props: Vec<PropDecl>,
    pub(super) fns: Vec<FnDecl>,
    prelude: Option<Prelude>,
    pub(super) generics: Vec<super::generics::GenericTemplate>,
    pub(super) quantifiers: Vec<super::quantifiers::Quantifiers>,
}

impl Definitions {
    pub fn pointer_width(&self) -> super::PointerWidth {
        self.pointer_width
    }
    pub fn with_pointer_width(pointer_width: super::PointerWidth) -> Self {
        Self {
            pointer_width,
            ..Self::default()
        }
    }

    /// No declarations at all, not even the prelude. Excluded middle is
    /// unavailable, because it has no `Or` and `False` to be stated with.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declarations starting with the prelude propositions:
    ///
    /// ~~~text
    /// prop True  { Intro }
    /// prop False { }
    /// prop And(p: Prop, q: Prop) { Intro(left: @p, right: @q) }
    /// prop Or(p: Prop, q: Prop)  { Left(@p), Right(@q) }
    /// ~~~
    pub fn with_prelude() -> (Self, Prelude) {
        Self::with_prelude_for(super::PointerWidth::HOST)
    }
    pub fn with_prelude_for(width: super::PointerWidth) -> (Self, Prelude) {
        let mut definitions = Self::with_pointer_width(width);
        let proof_of = |term: &Term| Type::proof(term.clone());
        let truth = definitions
            .declare_prop(vec![], vec![PropVariant::Params(Type::Tuple(vec![]))])
            .expect("prelude True");
        let falsehood = definitions
            .declare_prop(vec![], vec![])
            .expect("prelude False");
        let both = Type::tuple(|earlier| match earlier {
            [] | [_] => Some(Type::Prop),
            [p, _] => Some(proof_of(p)),
            [_, q, _] => Some(proof_of(q)),
            _ => None,
        });
        let and = definitions
            .declare_prop(
                vec![Type::Prop, Type::Prop],
                vec![PropVariant::Params(both)],
            )
            .expect("prelude And");
        let one_of = |which: usize| {
            Type::tuple(move |earlier| match earlier {
                [] | [_] => Some(Type::Prop),
                [p, q] => Some(proof_of(if which == 0 { p } else { q })),
                _ => None,
            })
        };
        let or = definitions
            .declare_prop(
                vec![Type::Prop, Type::Prop],
                vec![
                    PropVariant::Params(one_of(0)),
                    PropVariant::Params(one_of(1)),
                ],
            )
            .expect("prelude Or");
        let prelude = Prelude {
            truth,
            falsehood,
            and,
            or,
        };
        definitions.prelude = Some(prelude);
        (definitions, prelude)
    }

    pub fn prelude(&self) -> Option<Prelude> {
        self.prelude
    }

    /// Declares an enum with one payload tuple type per variant. Payloads
    /// must be well formed with no variables in scope.
    pub fn declare_enum(&mut self, variants: &[Type]) -> Result<EnumId, KernelError> {
        self.declare_enum_family(&[], variants)
    }

    pub fn declare_enum_family(
        &mut self,
        parameters: &[Type],
        variants: &[Type],
    ) -> Result<EnumId, KernelError> {
        // Bound borrowed input before cloning it into a telescope. Even a
        // rejected declaration must not recurse through an oversized type.
        check_depth(parameters.iter().map(Into::into))?;
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let signature = Type::Fn(parameters.to_vec(), Box::new(Type::Tuple(Vec::new())));
        check_depth([(&signature).into()])?;
        type_ok(&mut ctx, &signature)?;
        let mut payloads = Vec::new();
        for variant in variants {
            check_depth([variant.into()])?;
            let Type::Tuple(fields) = variant else {
                return Err(KernelError::NotAProduct(variant.clone()));
            };
            let signature = Type::Fn(parameters.to_vec(), Box::new(variant.clone()));
            check_depth([(&signature).into()])?;
            type_ok(&mut ctx, &signature)?;
            payloads.push(fields.clone());
        }
        self.enums.push(EnumDecl {
            parameters: parameters.to_vec(),
            variants: payloads,
            logical: false,
            group: Vec::new(),
        });
        Ok(EnumId(self.enums.len() - 1))
    }

    /// Declares a proposition by its parameters and its proof constructors.
    /// A parameter is data or a `Prop`, never a proof, so parameters do not
    /// depend on one another. Every variant concludes this proposition by
    /// construction: a conclusion is a list of arguments, not a proposition.
    pub fn declare_prop(
        &mut self,
        params: Vec<Type>,
        variants: Vec<PropVariant>,
    ) -> Result<PropId, KernelError> {
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        for param in &params {
            check_depth([param.into()])?;
            type_ok(&mut ctx, param)?;
            if matches!(param, Type::Proof(_)) {
                return Err(KernelError::ProofParameter(param.clone()));
            }
        }
        let mut decls = Vec::new();
        for variant in variants {
            decls.push(match variant {
                PropVariant::Arm { witnesses, body } => {
                    check_depth([(&witnesses).into(), (&body).into()])?;
                    let Type::Tuple(mut telescope) = witnesses else {
                        return Err(KernelError::NotAProduct(witnesses));
                    };
                    let prefix = telescope.get(..params.len()).unwrap_or(&[]);
                    if !same_types(prefix, &params) {
                        return Err(KernelError::FieldCount {
                            expected: params.len(),
                            found: prefix.len(),
                        });
                    }
                    for witness in &telescope[params.len()..] {
                        if matches!(witness, Type::Proof(_)) {
                            return Err(KernelError::ProofParameter(witness.clone()));
                        }
                    }
                    telescope.push(Type::proof(body));
                    check_telescope(&mut ctx, &telescope)?;
                    PropVariantDecl {
                        with_params: true,
                        telescope,
                        conclusion: Vec::new(),
                    }
                }
                PropVariant::Params(telescope) => {
                    check_depth([(&telescope).into()])?;
                    let Type::Tuple(telescope) = telescope else {
                        return Err(KernelError::NotAProduct(telescope));
                    };
                    let prefix = telescope.get(..params.len()).unwrap_or(&[]);
                    if !same_types(prefix, &params) {
                        return Err(KernelError::FieldCount {
                            expected: params.len(),
                            found: prefix.len(),
                        });
                    }
                    check_telescope(&mut ctx, &telescope)?;
                    PropVariantDecl {
                        with_params: true,
                        telescope,
                        conclusion: Vec::new(),
                    }
                }
                PropVariant::Indexed {
                    payload,
                    conclusion,
                } => {
                    check_depth(
                        std::iter::once((&payload).into()).chain(conclusion.iter().map(Into::into)),
                    )?;
                    let Type::Tuple(telescope) = payload else {
                        return Err(KernelError::NotAProduct(payload));
                    };
                    check_telescope(&mut ctx, &telescope)?;
                    if conclusion.len() != params.len() {
                        return Err(KernelError::FieldCount {
                            expected: params.len(),
                            found: conclusion.len(),
                        });
                    }
                    let scope = ctx.len();
                    let mut vars = Vec::new();
                    for index in 0..telescope.len() {
                        let ty = field_type(&telescope, index, |j| Term::Free(vars[j]));
                        vars.push(ctx.push_bound(ty));
                    }
                    let mut result = Ok(());
                    for (argument, param) in conclusion.iter().zip(&params) {
                        let argument = argument.instantiate(vars.len(), |j| Term::Free(vars[j]));
                        result = expect_type(&mut ctx, &argument, param, Mode::Logical);
                        if result.is_err() {
                            break;
                        }
                    }
                    ctx.truncate(scope);
                    result?;
                    PropVariantDecl {
                        with_params: false,
                        telescope,
                        conclusion,
                    }
                }
            });
        }
        self.props.push(PropDecl {
            params,
            variants: decls,
            inductive: false,
        });
        Ok(PropId(self.props.len() - 1))
    }

    /// Declares a struct with the fields of the given tuple type. The fields
    /// must be well formed with no variables in scope.
    pub fn declare_struct(&mut self, fields: &Type) -> Result<StructId, KernelError> {
        self.declare_struct_family(&[], fields)
    }

    pub fn declare_struct_family(
        &mut self,
        parameters: &[Type],
        fields: &Type,
    ) -> Result<StructId, KernelError> {
        // Bound borrowed input before cloning it into a telescope. Even a
        // rejected declaration must not recurse through an oversized type.
        check_depth(parameters.iter().map(Into::into))?;
        check_depth([fields.into()])?;
        let Type::Tuple(fields) = fields else {
            return Err(KernelError::NotAProduct(fields.clone()));
        };
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let signature = Type::Fn(parameters.to_vec(), Box::new(Type::Tuple(fields.clone())));
        check_depth([(&signature).into()])?;
        type_ok(&mut ctx, &signature)?;
        self.structs.push(StructDecl {
            parameters: parameters.to_vec(),
            fields: fields.clone(),
            logical: false,
        });
        Ok(StructId(self.structs.len() - 1))
    }

    /// Declares a math function of the given function type. `body(params)`
    /// receives the parameters as terms. The body is checked against the
    /// result type with no other variables in scope.
    pub fn declare_fn(
        &mut self,
        signature: &Type,
        body: impl FnOnce(&[Term]) -> Term,
    ) -> Result<FnId, KernelError> {
        check_depth([signature.into()])?;
        let Type::Fn(params, result) = signature else {
            return Err(KernelError::NotAFunction(signature.clone()));
        };
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let mut telescope = params.clone();
        telescope.push((**result).clone());
        check_telescope(&mut ctx, &telescope)?;

        // A parameter is ghost exactly when its type is. That makes no
        // difference to the logical check, and it is what the executable
        // check below needs.
        let mut vars = Vec::new();
        for index in 0..params.len() {
            let ty = field_type(&telescope, index, |j| Term::Free(vars[j]));
            let ghost = self.is_erased_type(&ty);
            vars.push(ctx.push_local(ty, ghost));
        }
        let arguments: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
        let body = body(&arguments);
        check_depth([(&body).into()])?;
        let expected = field_type(&telescope, params.len(), |j| Term::Free(vars[j]));
        expect_type(&mut ctx, &body, &expected, Mode::Logical)?;

        // The function has a runtime form only if its body is itself an
        // executable term. Otherwise a ghost argument could reach executable
        // data through the call: `f(n: Int) -> u8 { wrap[u8](n) }` would turn
        // a ghost number into a byte. Such a function is still a perfectly
        // good logical function; it is just not callable at runtime.
        let executable = !expected.is_ghost()
            && expect_type(&mut ctx, &body, &expected, Mode::Executable).is_ok();

        let classical = term_is_classical(self, &body);
        self.fns.push(FnDecl {
            params: params.clone(),
            result: (**result).clone(),
            body: body.close_over(&vars),
            classical,
            executable,
        });
        Ok(FnId(self.fns.len() - 1))
    }

    /// Erasure classification that also knows checked nominal declarations.
    /// Kernel bool remains mode-polymorphic; its surface Bool classification
    /// is carried by the elaborator rather than inferred from this type.
    pub fn is_erased_type(&self, ty: &Type) -> bool {
        ty.is_ghost()
            || match ty {
                Type::Struct(_) | Type::Enum(_) | Type::Instance(..) => self.is_logical_type(ty),
                Type::Fn(_, result) => self.is_erased_type(result),
                _ => false,
            }
    }

    /// Whether a kernel representation may satisfy a Logical bound. The
    /// surface checker separately distinguishes logical Bool from bool.
    pub fn is_logical_type(&self, ty: &Type) -> bool {
        match ty {
            Type::Instance(base, _) => self.is_logical_type(base),
            Type::Int | Type::Bool | Type::Prop | Type::Proof(_) => true,
            Type::Fn(_, result) => self.is_logical_type(result),
            Type::Struct(id) => self.structs.get(id.0).is_some_and(|decl| decl.logical),
            Type::Enum(id) => self.enums.get(id.0).is_some_and(|decl| decl.logical),
            Type::U8 | Type::Machine(_) | Type::Tuple(_) | Type::Boxed(_) | Type::Buffer(_) => {
                false
            }
        }
    }

    /// Register an aggregate as logical only after checking every field.
    /// This is checked derivation, never an unchecked user assertion.
    pub fn mark_logical(&mut self, ty: &Type) -> Result<(), KernelError> {
        let fields: Vec<&Type> = match ty {
            Type::Struct(id) => self
                .structs
                .get(id.0)
                .ok_or(KernelError::UnknownStruct)?
                .fields
                .iter()
                .collect(),
            Type::Enum(id) => self
                .enums
                .get(id.0)
                .ok_or(KernelError::UnknownEnum)?
                .variants
                .iter()
                .flatten()
                .collect(),
            _ if self.is_logical_type(ty) => return Ok(()),
            _ => return Err(KernelError::NotLogicalType(ty.clone())),
        };
        if let Some(field) = fields
            .into_iter()
            .find(|field| !self.is_logical_type(field))
        {
            return Err(KernelError::NotLogicalType(field.clone()));
        }
        match ty {
            Type::Struct(id) => self.structs[id.0].logical = true,
            Type::Enum(id) => self.enums[id.0].logical = true,
            _ => unreachable!("only checked aggregates reach registration"),
        }
        Ok(())
    }

    /// Restrict an accepted declaration to logical use. This only removes
    /// runtime permission; the body has already been checked as a total term.
    pub fn restrict_to_logic(&mut self, id: FnId) -> Result<(), KernelError> {
        self.fns
            .get_mut(id.0)
            .ok_or(KernelError::UnknownFunction)?
            .executable = false;
        Ok(())
    }

    /// Whether the function's body uses excluded middle, directly or through
    /// the functions it calls.
    pub fn is_classical(&self, id: FnId) -> bool {
        self.fns.get(id.0).is_some_and(|decl| decl.classical)
    }

    /// Whether the function may be named in executable code. A function whose
    /// result is ghost, or whose body needs a ghost value to compute its
    /// result, is logical-only.
    pub fn is_executable(&self, id: FnId) -> bool {
        self.fns.get(id.0).is_some_and(|decl| decl.executable)
    }

    /// A declared function's type.
    pub fn signature(&self, id: FnId) -> Option<Type> {
        self.fns
            .get(id.0)
            .map(|decl| Type::Fn(decl.params.clone(), Box::new(decl.result.clone())))
    }

    /// A declared function's arity and body, for an interpreter. The body is
    /// under one binder per parameter: the last parameter is `Bound(0)`.
    pub fn function_body(&self, id: FnId) -> Option<(usize, &Term)> {
        self.fns
            .get(id.0)
            .map(|decl| (decl.params.len(), &decl.body))
    }

    pub fn family_parameters(&self, base: &Type) -> Option<&[Type]> {
        match base {
            Type::Struct(id) => self.structs.get(id.0).map(|d| d.parameters.as_slice()),
            Type::Enum(id) => self.enums.get(id.0).map(|d| d.parameters.as_slice()),
            _ => None,
        }
    }

    fn instantiate_payload(&self, ty: &Type, fields: &[Type]) -> Option<Vec<Type>> {
        let parameters = self.family_parameters(ty.nominal())?;
        if parameters.len() != ty.indices().len() {
            return None;
        }
        let mut telescope = parameters.to_vec();
        telescope.push(Type::Tuple(fields.to_vec()));
        let Type::Tuple(fields) =
            field_type(&telescope, parameters.len(), |i| ty.indices()[i].clone())
        else {
            unreachable!()
        };
        Some(fields)
    }

    pub fn instance_fields(&self, ty: &Type) -> Option<Vec<Type>> {
        let Type::Struct(id) = ty.nominal() else {
            return None;
        };
        self.instantiate_payload(ty, self.struct_fields(*id)?)
    }

    pub fn instance_variants(&self, ty: &Type) -> Option<Vec<Vec<Type>>> {
        let Type::Enum(id) = ty.nominal() else {
            return None;
        };
        if self.family_parameters(ty.nominal())?.len() != ty.indices().len() {
            return None;
        }
        self.enum_variants(*id)?
            .iter()
            .map(|fields| self.instantiate_payload(ty, fields))
            .collect()
    }

    pub(super) fn enum_variants(&self, id: EnumId) -> Option<&[Vec<Type>]> {
        self.enums.get(id.0).map(|decl| decl.variants.as_slice())
    }

    pub(super) fn prop(&self, id: PropId) -> Option<&PropDecl> {
        self.props.get(id.0)
    }

    pub(super) fn function(&self, id: FnId) -> Option<&FnDecl> {
        self.fns.get(id.0)
    }

    pub(super) fn struct_fields(&self, id: StructId) -> Option<&[Type]> {
        self.structs.get(id.0).map(|decl| decl.fields.as_slice())
    }
}
