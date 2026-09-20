//! Global declarations: structs, enums, propositions, and math functions.
//!
//! A declaration is checked against the declarations that precede it, so a
//! struct cannot mention itself and a function cannot call itself, directly or
//! through other declarations. With no loops in kernel terms, that makes
//! every declared function total.

use std::rc::Rc;

use super::check::{check_telescope, check_type, expect_type, same_types};
use super::classical::term_is_classical;
use super::context::{Context, Mode};
use super::error::KernelError;
use super::term::{EnumId, FnId, PropId, StructId, Term, Type, VarId, field_type};

#[derive(Clone, Debug)]
pub(super) struct StructDecl {
    /// A telescope, as in `Type::Tuple`.
    pub(super) fields: Vec<Type>,
}

#[derive(Clone, Debug)]
pub(super) struct EnumDecl {
    /// One payload telescope per variant.
    pub(super) variants: Vec<Vec<Type>>,
}

/// A variant of a declared proposition, as given to `declare_prop`.
#[derive(Clone, Debug)]
pub enum PropVariant {
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
}

#[derive(Clone, Debug, Default)]
pub struct Definitions {
    structs: Vec<StructDecl>,
    enums: Vec<EnumDecl>,
    props: Vec<PropDecl>,
    fns: Vec<FnDecl>,
    prelude: Option<Prelude>,
}

impl Definitions {
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
        let mut definitions = Self::default();
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
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let mut payloads = Vec::new();
        for variant in variants {
            let Type::Tuple(fields) = variant else {
                return Err(KernelError::NotAProduct(variant.clone()));
            };
            check_telescope(&mut ctx, fields)?;
            payloads.push(fields.clone());
        }
        self.enums.push(EnumDecl { variants: payloads });
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
            check_type(&mut ctx, param)?;
            if matches!(param, Type::Proof(_)) {
                return Err(KernelError::ProofParameter(param.clone()));
            }
        }
        let mut decls = Vec::new();
        for variant in variants {
            decls.push(match variant {
                PropVariant::Params(telescope) => {
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
        });
        Ok(PropId(self.props.len() - 1))
    }

    /// Declares a struct with the fields of the given tuple type. The fields
    /// must be well formed with no variables in scope.
    pub fn declare_struct(&mut self, fields: &Type) -> Result<StructId, KernelError> {
        let Type::Tuple(fields) = fields else {
            return Err(KernelError::NotAProduct(fields.clone()));
        };
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        check_telescope(&mut ctx, fields)?;
        self.structs.push(StructDecl {
            fields: fields.clone(),
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
        let Type::Fn(params, result) = signature else {
            return Err(KernelError::NotAFunction(signature.clone()));
        };
        let mut ctx = Context::with_definitions(Rc::new(self.clone()));
        let mut telescope = params.clone();
        telescope.push((**result).clone());
        check_telescope(&mut ctx, &telescope)?;

        let mut vars = Vec::new();
        for index in 0..params.len() {
            let ty = field_type(&telescope, index, |j| Term::Free(vars[j]));
            vars.push(ctx.push_bound(ty));
        }
        let arguments: Vec<Term> = vars.iter().copied().map(Term::Free).collect();
        let body = body(&arguments);
        let expected = field_type(&telescope, params.len(), |j| Term::Free(vars[j]));
        expect_type(&mut ctx, &body, &expected, Mode::Logical)?;

        let classical = term_is_classical(self, &body);
        self.fns.push(FnDecl {
            params: params.clone(),
            result: (**result).clone(),
            body: body.close_over(&vars),
            classical,
        });
        Ok(FnId(self.fns.len() - 1))
    }

    /// Whether the function's body uses excluded middle, directly or through
    /// the functions it calls.
    pub fn is_classical(&self, id: FnId) -> bool {
        self.fns.get(id.0).is_some_and(|decl| decl.classical)
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
