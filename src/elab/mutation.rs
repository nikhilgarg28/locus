//! `let mut`, assignment, and the join of a branch that assigns.
//!
//! A mutable binding is a `Local` whose `binding` is its own identity. An
//! assignment gives it a new version, a fresh identity defined in the
//! mirrored context as the old version with the assigned path replaced,
//! exactly the term lowering states (`typed::rebuilt`), and the `Local`'s
//! `id` becomes that version in place, so that the rest of the block, and
//! the blocks around it, mention the new version. A branch's arms work on
//! the versions they find and are put back afterwards; what they assigned
//! reaches the code after the branch through the join, which mirrors the
//! tuple lowering builds (`typed::join_type`) and gives every assigned
//! binding one more version, bound by projection.
//!
//! Lowering computes the set of assigned bindings for itself and rejects a
//! join that names any other set. Nothing here is trusted: a wrong version
//! in a proof is a proof the kernel rejects, and a wrong version in an
//! executable position is a stale mention lowering refuses.
//!
//! Tracked evidence, `let mut ok: @P`, is M4's. Until then a proposition or
//! a type that mentions a mutable binding names the version current where
//! it was written: a snapshot.

use crate::ast::{self, ExprKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::{HypId, Proof, Term, Type, VarId, same, same_type};
use crate::source::Span;
use crate::typed::{
    Binder, Join, Joined, Named, Place, Step, Stmt, join_type, opened_part, opened_type, rebuilt,
};

use super::env::{Elab, Env, Fact, Mark};

/// The mutable bindings in scope at the entry of a branch: each one's slot
/// among the names, its identity, and its version at entry.
pub(super) struct Entry(Vec<(usize, VarId, VarId)>);

/// What one arm of a branch ended with: the version each entry binding had,
/// the type of the arm's value, and whether the arm transfers control.
pub(super) struct ArmEnd {
    pub versions: Vec<VarId>,
    pub ty: Type,
    pub never: bool,
}

/// One field of the left side of an assignment, as written.
enum Part<'a> {
    Field(&'a ast::Name),
    Index(&'a str, Span),
}

/// The name at the root of a place and the fields after it, or `None` for
/// anything else, which the parser already refused.
fn place_path(expr: &ast::Expr) -> Option<(&ast::Name, Vec<Part<'_>>)> {
    match &expr.kind {
        ExprKind::Name(name) => Some((name, Vec::new())),
        ExprKind::Member { value, name } => {
            let (root, mut parts) = place_path(value)?;
            parts.push(Part::Field(name));
            Some((root, parts))
        }
        ExprKind::Index {
            value,
            index,
            index_span,
        } => {
            let (root, mut parts) = place_path(value)?;
            parts.push(Part::Index(index, *index_span));
            Some((root, parts))
        }
        _ => None,
    }
}

/// Whether a type mentions a context variable.
pub(super) fn type_mentions(ty: &Type, id: VarId) -> bool {
    let wanted = Term::var(id);
    match ty {
        Type::Proof(claim) => claim.find(&|term| same(term, &wanted)).is_some(),
        Type::Tuple(fields) => fields.iter().any(|field| type_mentions(field, id)),
        Type::Fn(params, result) => {
            params.iter().any(|param| type_mentions(param, id)) || type_mentions(result, id)
        }
        _ => false,
    }
}

impl Env<'_> {
    /// Marks the name just bound as declared `let mut`.
    pub(super) fn make_mutable(&mut self, id: VarId) {
        if let Some(local) = self.names.iter_mut().rev().find(|local| local.id == id) {
            local.binding = Some(id);
        }
    }

    /// `place = value;`
    pub(super) fn assign_statement(
        &mut self,
        place: &ast::Expr,
        value: &ast::Expr,
        span: Span,
    ) -> Elab<Stmt> {
        let Some((root, parts)) = place_path(place) else {
            return self.internal("an assignment to something that is not a place", place.span);
        };
        let Some(slot) = self.names.iter().rposition(|local| local.name == root.text) else {
            return self.fail("L0204", format!("unknown name `{}`", root.text), root.span);
        };
        let local = &self.names[slot];
        if local.poisoned {
            return Err(());
        }
        let Some(binding) = local.binding else {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0232",
                    format!("cannot assign twice to immutable variable `{}`", root.text),
                    root.span,
                )
                .note(format!(
                    "a binding is assigned only when declared with `let mut {}`; a parameter cannot be assigned yet",
                    root.text
                )),
            );
            return Err(());
        };
        if local.depth < self.loops.len() {
            return self.fail(
                "L0290",
                format!(
                    "assignment to `{}` inside a loop body, where it was declared outside, is not in Locus yet; M3 replaces the loop forms",
                    root.text
                ),
                span,
            );
        }
        let name = local.name.clone();
        let declared = local.ty.clone();
        let mut target = Term::var(local.id);
        let mut ty = declared.clone();
        let mut steps = Vec::new();
        for part in parts {
            let (index, field_name, part_span) = match (&ty, &part) {
                (Type::Struct(id), Part::Field(field)) => {
                    let info = self.struct_by_id(*id).expect("a struct type was declared");
                    let Some(index) = info.fields.iter().position(|f| f.name == field.text) else {
                        let message = format!("`{}` has no field `{}`", info.name, field.text);
                        return self.fail("L0210", message, field.span);
                    };
                    (index, Some(field.text.clone()), field.span)
                }
                (Type::Tuple(fields), Part::Index(index, index_span)) => {
                    match index.parse::<usize>() {
                        Ok(position) if position < fields.len() => (position, None, *index_span),
                        _ => {
                            let shown = self.show_type(&ty);
                            return self.fail(
                                "L0210",
                                format!("`{shown}` has no field `{index}`"),
                                *index_span,
                            );
                        }
                    }
                }
                (_, Part::Field(field)) => {
                    let shown = self.show_type(&ty);
                    return self.fail(
                        "L0210",
                        format!("`{shown}` has no field `{}`", field.text),
                        field.span,
                    );
                }
                (_, Part::Index(index, index_span)) => {
                    let shown = self.show_type(&ty);
                    return self.fail(
                        "L0210",
                        format!("`{shown}` has no field `{index}`"),
                        *index_span,
                    );
                }
            };
            let proof_fields = self.proof_fields(&ty);
            // A later field whose type speaks of this one would be carried
            // over as evidence about the old value.
            let assigned = Term::proj(target.clone(), index);
            for later in index + 1..proof_fields.len() {
                let later_ty = self.type_of(&Term::proj(target.clone(), later), part_span)?;
                let Type::Proof(claim) = &later_ty else {
                    continue;
                };
                if claim.find(&|term| same(term, &assigned)).is_some() {
                    let (assigned, dependent) = (
                        self.show_path(&name, &steps, &field_name, index),
                        self.show_path(&name, &steps, &self.field_name(&ty, later), later),
                    );
                    self.diagnostics.push(
                        Diagnostic::error(
                            "L0233",
                            format!(
                                "assigning `{assigned}` alone would invalidate `{dependent}`; replace the whole value"
                            ),
                            part_span,
                        )
                        .note("evidence in a field speaks of the fields before it, and a value must satisfy its evidence at every moment"),
                    );
                    return Err(());
                }
            }
            steps.push(Step {
                index,
                name: field_name,
                ty: ty.clone(),
                proof_fields,
            });
            target = assigned;
            ty = self.type_of(&target, part_span)?;
        }
        let value = self.check(value, &ty)?;
        let value_term = self.term(&value, span)?;
        // The right side may have assigned the binding itself: the place is
        // rebuilt from the version current after it.
        let current = self.names[slot].id;
        let whole = match rebuilt(Term::var(current), &steps, value_term) {
            Ok(whole) => whole,
            Err(error) => return self.internal(error, span),
        };
        let (version, equation) = (VarId::fresh(), HypId::fresh());
        let defined = self.ctx.define_with(version, equation, &whole);
        let found = self.kernel(defined, span)?;
        if !same_type(&found, &declared) {
            let (wanted, found) = (self.show_type(&declared), self.show_type(&found));
            return self.internal(
                format!("the assigned value has type `{found}`, and `{name}` is `{wanted}`"),
                span,
            );
        }
        if !matches!(found, Type::Proof(_)) {
            self.facts.push(Fact::definition(
                Proof::hyp(equation),
                Term::eq(found.clone(), Term::var(version), whole),
            ));
        }
        self.names[slot].id = version;
        self.labels.insert(version, name.clone());
        self.learn_from(&Term::var(version), &found);
        Ok(Stmt::Assign {
            place: Place {
                binding,
                name: name.clone(),
                path: steps,
            },
            value: value.expr,
            version: Binder {
                id: version,
                name,
                ty: declared,
            },
            equation,
        })
    }

    /// Which fields of a product type hold evidence; the length is its arity.
    fn proof_fields(&self, ty: &Type) -> Vec<bool> {
        match ty {
            Type::Tuple(fields) => fields
                .iter()
                .map(|field| matches!(field, Type::Proof(_)))
                .collect(),
            Type::Struct(id) => self.struct_by_id(*id).map_or_else(Vec::new, |info| {
                info.fields
                    .iter()
                    .map(|field| matches!(field.ty, Type::Proof(_)))
                    .collect()
            }),
            _ => Vec::new(),
        }
    }

    fn field_name(&self, ty: &Type, index: usize) -> Option<String> {
        match ty {
            Type::Struct(id) => self
                .struct_by_id(*id)
                .and_then(|info| info.fields.get(index).map(|field| field.name.clone())),
            _ => None,
        }
    }

    /// `name.f.0.g`, for a message.
    fn show_path(&self, name: &str, steps: &[Step], last: &Option<String>, index: usize) -> String {
        let mut shown = name.to_string();
        for step in steps {
            shown.push('.');
            match &step.name {
                Some(name) => shown.push_str(name),
                None => shown.push_str(&step.index.to_string()),
            }
        }
        shown.push('.');
        match last {
            Some(name) => shown.push_str(name),
            None => shown.push_str(&index.to_string()),
        }
        shown
    }

    // --- The join of a branch ---

    pub(super) fn mutable_entry(&self) -> Entry {
        Entry(
            self.names
                .iter()
                .enumerate()
                .filter_map(|(slot, local)| local.binding.map(|binding| (slot, binding, local.id)))
                .collect(),
        )
    }

    /// The versions the entry bindings have now: at the end of an arm,
    /// before its scope closes.
    pub(super) fn versions_now(&self, entry: &Entry) -> Vec<VarId> {
        entry
            .0
            .iter()
            .map(|(slot, _, _)| self.names[*slot].id)
            .collect()
    }

    /// The facts a block added, when it assigned an entry binding: they
    /// speak of identities that stay declared, and what the block assigned
    /// is what the code after it reads.
    pub(super) fn facts_since(&self, mark: &Mark, entry: &Entry) -> Vec<Fact> {
        if self.versions_now(entry) == entry.0.iter().map(|(_, _, v)| *v).collect::<Vec<_>>() {
            return Vec::new();
        }
        self.facts[mark.facts()..].to_vec()
    }

    /// Puts the entry versions back once an arm's scope has closed: what
    /// the arm assigned reaches the code after the branch through the join
    /// alone.
    pub(super) fn restore_versions(&mut self, entry: &Entry) {
        for (slot, _, version) in &entry.0 {
            self.names[*slot].id = *version;
        }
    }

    /// Whether an arm's value type mentions a version the arm made, which
    /// no other arm can be checked against.
    pub(super) fn mentions_arm_version(entry: &Entry, versions: &[VarId], ty: &Type) -> bool {
        entry
            .0
            .iter()
            .zip(versions)
            .any(|((_, _, at_entry), now)| now != at_entry && type_mentions(ty, *now))
    }

    /// Joins the arms of a branch. When no arm assigned an entry binding,
    /// nothing happens and the type is `fallback`, the one the branch has
    /// today. Otherwise every assigned binding gets a version for after the
    /// branch, the value's type is stated over those versions, and the
    /// mirrored context binds the tuple's parts as lowering will.
    pub(super) fn join(
        &mut self,
        entry: &Entry,
        arms: &[ArmEnd],
        fallback: Type,
        result: VarId,
        span: Span,
    ) -> Elab<(Option<Joined>, Type)> {
        let assigned: Vec<usize> = (0..entry.0.len())
            .filter(|&i| arms.iter().any(|arm| arm.versions[i] != entry.0[i].2))
            .collect();
        if assigned.is_empty() {
            return Ok((None, fallback));
        }
        let joins: Vec<Join> = assigned
            .iter()
            .map(|&i| {
                let (slot, binding, _) = entry.0[i];
                let local = &self.names[slot];
                Join {
                    binding,
                    version: Binder {
                        id: VarId::fresh(),
                        name: local.name.clone(),
                        ty: local.ty.clone(),
                    },
                    equation: HypId::fresh(),
                }
            })
            .collect();
        // An arm's value is typed over the versions it ended with; over the
        // joined versions it is the type of the whole. When the arms agree
        // only as written, the type is a snapshot of what they wrote.
        let renamed = |arm: &ArmEnd| {
            joins
                .iter()
                .zip(&assigned)
                .fold(arm.ty.clone(), |ty, (join, &i)| {
                    ty.replace_var(arm.versions[i], &Term::var(join.version.id))
                })
        };
        let live: Vec<&ArmEnd> = arms.iter().filter(|arm| !arm.never).collect();
        let ty = match live.split_first() {
            None => fallback,
            Some((first, rest)) => {
                let candidate = renamed(first);
                if rest.iter().all(|arm| same_type(&renamed(arm), &candidate)) {
                    candidate
                } else if rest.iter().all(|arm| same_type(&arm.ty, &first.ty)) {
                    first.ty.clone()
                } else {
                    return self.fail(
                        "L0220",
                        "the arms of this branch produce values of different types over what they assign",
                        span,
                    );
                }
            }
        };
        let joined = Joined {
            tuple: VarId::fresh(),
            joins,
            equation: HypId::fresh(),
        };
        let declared = self
            .ctx
            .declare_with(joined.tuple, join_type(&joined, result, &ty), false);
        self.kernel(declared, span)?;
        let label = self
            .text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.labels.insert(joined.tuple, label.clone());
        let tuple = Term::var(joined.tuple);
        let mut earlier: Vec<Named> = Vec::new();
        for (index, (join, &i)) in joined.joins.iter().zip(&assigned).enumerate() {
            let found = self.bind_part(
                &tuple,
                index,
                join.version.id,
                join.equation,
                &mut earlier,
                span,
            )?;
            let slot = entry.0[i].0;
            self.names[slot].id = join.version.id;
            self.labels
                .insert(join.version.id, join.version.name.clone());
            self.learn_from(&Term::var(join.version.id), &found);
        }
        let found = self.bind_part(
            &tuple,
            joined.joins.len(),
            result,
            joined.equation,
            &mut earlier,
            span,
        )?;
        if !same_type(&found, &ty) {
            let (wanted, found) = (self.show_type(&ty), self.show_type(&found));
            return self.internal(
                format!("the joined value has type `{found}`, and `{wanted}` was expected"),
                span,
            );
        }
        self.labels.insert(result, label);
        self.learn_from(&Term::var(result), &ty);
        Ok((Some(joined), ty))
    }

    /// Binds one part of the join's tuple, as a `let` pattern binds a part
    /// of a tuple: over the names bound before it. Returns its type.
    fn bind_part(
        &mut self,
        tuple: &Term,
        index: usize,
        id: VarId,
        equation: HypId,
        earlier: &mut Vec<Named>,
        span: Span,
    ) -> Elab<Type> {
        let part = Term::proj(tuple.clone(), index);
        let over_projections = self.type_of(&part, span)?;
        let opened = opened_type(&over_projections, earlier);
        let value = opened_part(&part, &opened, earlier);
        let defined = self.ctx.define_with(id, equation, &value);
        let ty = self.kernel(defined, span)?;
        if !matches!(ty, Type::Proof(_)) {
            self.facts.push(Fact::definition(
                Proof::hyp(equation),
                Term::eq(ty.clone(), Term::var(id), value.clone()),
            ));
            earlier.push(Named {
                id,
                equation,
                ty: ty.clone(),
                value,
            });
        }
        Ok(ty)
    }
}
