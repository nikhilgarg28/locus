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
//! Tracked evidence, `let mut ok: @P`, is a mutable binding whose declared
//! type mentions other mutable bindings (`Tracked`). Its type is fixed as
//! written, over those bindings and not over their versions: at each point
//! the type of its current version is `P` over the versions current there
//! (`version_type`), which is what an assignment to it is checked against,
//! what its version in a join's tuple or a loop's state has, and what a
//! use of it produces. The flow analysis here says when it is available:
//! an assignment to a binding it mentions makes it stale until an
//! assignment to it, `ok = _;` or `ok = proof;`, refreshes it; after a
//! branch it is stale when it is stale at the end of any arm that reaches
//! the join, and a branch joins it only when every such arm leaves it
//! valid; a loop carries it when it is valid at entry, needs it valid at
//! every `continue` and `break` and at the end of the body, and gives it
//! back valid (`loops.rs`). A use of a stale one is an error shaped like a
//! use after move, naming the assignment. None of this is trusted:
//! lowering gives each version its type over the versions current there,
//! and a stale use is a version of another type than the one wanted, which
//! the checker rejects. A snapshot, evidence bound with `let`, keeps its
//! type as written.
//!
//! A call with a `&mut` argument assigns the argument's root: after the
//! call the root gets a new version, the old one with the lent path
//! replaced by the value the callee returned for it, through the same
//! `write_back` an assignment uses, with the same effect on tracked
//! evidence (`references.rs`). The right side of an assignment may be such
//! a call, so the type the assigned place has is read again after the
//! right side, from the version current then.

use crate::ast::{self, ExprKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::{HypId, Proof, Term, Type, VarId, same, same_type};
use crate::source::Span;
use crate::typed::{
    Binder, Join, Joined, Named, Place, Step, Stmt, join_type, opened_part, opened_type, rebuilt,
};

use super::env::{Elab, Env, Fact, Mark};

/// Tracked evidence: what its declared type mentions, and whether it is
/// valid here.
#[derive(Clone, Debug)]
pub(super) struct Tracked {
    /// The mutable bindings the declared type mentions, by identity, each
    /// with the version of it the type was written over.
    pub deps: Vec<(VarId, VarId)>,
    /// `None` while the evidence is valid; otherwise what made it stale.
    pub stale: Option<Stale>,
}

/// Why tracked evidence is not available: the binding it mentions that was
/// changed, and the assignment, the `&mut` call, or the loop that changed
/// it.
#[derive(Clone, Debug)]
pub(super) struct Stale {
    pub dep: String,
    pub span: Span,
    pub by_loop: bool,
    /// Lent by `&mut` to a call, which assigned it.
    pub lent: bool,
}

/// How a place is reached: assigned, lent by `&mut`, or lent by `&`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Access {
    Assign,
    LendMut,
    Lend,
    /// Read by value, as a plain argument.
    Read,
}

/// The value a join produces: its type when no path reaches the join, and
/// the identity and equation it is bound under when the join has one, a
/// `loop`'s value or a branch's.
pub(super) struct JoinValue {
    pub fallback: Type,
    pub bound: Option<(VarId, HypId)>,
}

/// One mutable binding at the entry of a branch or a loop: its slot among
/// the names, its identity, its version at entry, and for tracked evidence
/// whether it was valid there.
#[derive(Clone)]
struct EntryBinding {
    slot: usize,
    binding: VarId,
    version: VarId,
    stale: Option<Stale>,
}

/// The mutable bindings in scope at the entry of a branch or a loop.
#[derive(Clone)]
pub(super) struct Entry(Vec<EntryBinding>);

impl Entry {
    /// The positions in the entry of the bindings at the given slots.
    pub fn positions(&self, slots: &[usize]) -> Vec<usize> {
        (0..self.0.len())
            .filter(|&i| slots.contains(&self.0[i].slot))
            .collect()
    }

    /// The versions the entry bindings had at entry.
    fn versions(&self) -> Vec<VarId> {
        self.0.iter().map(|entry| entry.version).collect()
    }
}

/// What one arm of a branch, or one exit of a loop, ended with: the version
/// each entry binding had and whether it was valid, the type of the value,
/// whether the arm transfers control, and whether it does so by the shape
/// of its tree (`typed::block_leaves`), in which case what it assigned is
/// not joined, as lowering builds no tuple from it.
#[derive(Clone)]
pub(super) struct ArmEnd {
    pub versions: Vec<VarId>,
    pub stale: Vec<Option<Stale>>,
    pub ty: Type,
    pub never: bool,
    pub leaves: bool,
}

/// One field of a place, as written.
pub(super) enum Part<'a> {
    Field(&'a ast::Name),
    Index(&'a str, Span),
}

/// The name at the root of a place and the fields after it, or `None` for
/// anything else: the parser refuses it on the left of `=`, and a lend
/// reports it (`references.rs`).
pub(super) fn place_path(expr: &ast::Expr) -> Option<(&ast::Name, Vec<Part<'_>>)> {
    match &expr.kind {
        ExprKind::Name(name) => Some((name, Vec::new())),
        ExprKind::Group(inner) => place_path(inner),
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
    /// Marks the name just bound as declared `let mut`. When its type
    /// mentions other mutable bindings it is tracked evidence, valid as
    /// declared.
    pub(super) fn make_mutable(&mut self, id: VarId) {
        let Some(slot) = self.names.iter().rposition(|local| local.id == id) else {
            return;
        };
        let deps: Vec<(VarId, VarId)> = self
            .names
            .iter()
            .filter(|local| local.id != id)
            .filter_map(|local| {
                let binding = local.binding?;
                type_mentions(&self.names[slot].ty, local.id).then_some((binding, local.id))
            })
            .collect();
        let local = &mut self.names[slot];
        local.binding = Some(id);
        if !deps.is_empty() {
            local.tracked = Some(Tracked { deps, stale: None });
        }
    }

    /// The current version of a mutable binding, by identity.
    pub(super) fn current_version(&self, binding: VarId) -> Option<VarId> {
        self.names
            .iter()
            .rev()
            .find(|local| local.binding == Some(binding))
            .map(|local| local.id)
    }

    /// The type of the binding at `slot` here: as declared, and for tracked
    /// evidence over the current versions of what it mentions. This is the
    /// type lowering gives each version (`typed::lower`).
    pub(super) fn version_type(&self, slot: usize) -> Type {
        let local = &self.names[slot];
        let Some(tracked) = &local.tracked else {
            return local.ty.clone();
        };
        tracked
            .deps
            .iter()
            .fold(local.ty.clone(), |ty, &(binding, written)| {
                match self.current_version(binding) {
                    Some(current) if current != written => {
                        ty.replace_var(written, &Term::var(current))
                    }
                    _ => ty,
                }
            })
    }

    /// An assignment to `binding`, or a loop that assigns it, makes every
    /// tracked evidence that mentions it stale.
    fn invalidate(&mut self, binding: VarId, stale: &Stale) {
        for local in &mut self.names {
            if let Some(tracked) = &mut local.tracked
                && tracked.deps.iter().any(|(dep, _)| *dep == binding)
            {
                tracked.stale = Some(stale.clone());
            }
        }
    }

    fn line_of(&self, span: Span) -> usize {
        self.source
            .line_column(span.start)
            .map_or(0, |(line, _)| line)
    }

    /// The error for tracked evidence that is stale where it is needed:
    /// `L0245` at a use, `L0246` at a point of control where it must be
    /// valid, with `when` saying which. Shaped like a use after move.
    pub(super) fn stale_error<T>(
        &mut self,
        slot: usize,
        stale: &Stale,
        code: &'static str,
        when: &str,
        span: Span,
    ) -> Elab<T> {
        let name = self.names[slot].name.clone();
        let line = self.line_of(stale.span);
        let (changed, label) = if stale.by_loop {
            (
                format!("which the loop at line {line} assigns"),
                format!("this loop assigns `{}`", stale.dep),
            )
        } else if stale.lent {
            (
                format!("which was lent by `&mut` at line {line}"),
                format!("the call assigns `{}`", stale.dep),
            )
        } else {
            (
                format!("which was assigned at line {line}"),
                format!("`{}` is assigned here", stale.dep),
            )
        };
        self.diagnostics.push(
            Diagnostic::error(
                code,
                format!(
                    "`{name}` speaks of `{}`, {changed}; refresh it with `{name} = _;` or `{name} = <proof>;` {when}",
                    stale.dep
                ),
                span,
            )
            .label(stale.span, label)
            .note(format!(
                "`{name}` is tracked evidence, declared with `let mut`: it speaks of `{}` as it is now, and an assignment leaves it to be established again",
                stale.dep
            )),
        );
        Err(())
    }

    /// Every tracked evidence among `slots` must be valid here: at a point
    /// of control where lowering supplies it at the current versions.
    pub(super) fn require_valid(&mut self, slots: &[usize], when: &str, span: Span) -> Elab<()> {
        let first_stale = slots.iter().find_map(|&slot| {
            let stale = self.names[slot].tracked.as_ref()?.stale.clone()?;
            Some((slot, stale))
        });
        match first_stale {
            Some((slot, stale)) => self.stale_error(slot, &stale, "L0246", when, span),
            None => Ok(()),
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
        let slot = self.place_slot(root)?;
        let Some(binding) = self.names[slot].binding else {
            self.diagnostics.push(
                Diagnostic::error(
                    "L0232",
                    format!("cannot assign twice to immutable variable `{}`", root.text),
                    root.span,
                )
                .note(format!(
                    "a binding is assigned only when declared with `let mut {0}`, or as a parameter `mut {0}: T` or `{0}: &mut T`",
                    root.text
                )),
            );
            return Err(());
        };
        let name = self.names[slot].name.clone();
        let (steps, ty) = self.place_steps(slot, &parts, Access::Assign)?;
        // The right side, against the type the place has now. A right side
        // that gives the root a new version, a `&mut` call, leaves the
        // place to be read from the version current afterwards, so the
        // value is accepted against the type it has then.
        let checked = self.expr(value, Some(&ty))?;
        let ty = self.place_type(slot, &steps, span)?;
        let value = self.coerce(checked, &ty, value.span)?;
        let value_term = self.term(&value, span)?;
        let (version, equation) =
            self.write_back(slot, &steps, value_term, Access::Assign, span)?;
        Ok(Stmt::Assign {
            place: Place {
                binding,
                name,
                path: steps,
            },
            value: value.expr,
            version,
            equation,
        })
    }

    /// The local a place is rooted at, by its name.
    pub(super) fn place_slot(&mut self, root: &ast::Name) -> Elab<usize> {
        let Some(slot) = self.names.iter().rposition(|local| local.name == root.text) else {
            return self.fail("L0204", format!("unknown name `{}`", root.text), root.span);
        };
        if self.names[slot].poisoned {
            return Err(());
        }
        Ok(slot)
    }

    /// The type of a place at the current version of its root: for the
    /// root itself, its type here, which for tracked evidence is over the
    /// current versions of what it mentions; for a field, the kernel's.
    pub(super) fn place_type(&mut self, slot: usize, steps: &[Step], span: Span) -> Elab<Type> {
        if steps.is_empty() {
            return Ok(self.version_type(slot));
        }
        let target = steps
            .iter()
            .fold(Term::var(self.names[slot].id), |target, step| {
                Term::proj(target, step.index)
            });
        self.type_of(&target, span)
    }

    /// Walks the path of a place from the local at `slot`, at its current
    /// version: the steps, with what rebuilding the product around each
    /// needs, and the type of the place. A field that evidence in the same
    /// product depends on cannot be assigned or lent by `&mut` alone
    /// (`L0233`): the rebuilt value would carry evidence about the old
    /// field, and a callee would write the field before anything is
    /// checked again.
    pub(super) fn place_steps(
        &mut self,
        slot: usize,
        parts: &[Part<'_>],
        access: Access,
    ) -> Elab<(Vec<Step>, Type)> {
        let name = self.names[slot].name.clone();
        let local = &self.names[slot];
        let mut target = Term::var(local.id);
        let mut ty = self.version_type(slot);
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
                if access == Access::Lend {
                    break;
                }
                let later_ty = self.type_of(&Term::proj(target.clone(), later), part_span)?;
                let Type::Proof(claim) = &later_ty else {
                    continue;
                };
                if claim.find(&|term| same(term, &assigned)).is_some() {
                    let (assigned, dependent) = (
                        self.show_path(&name, &steps, &field_name, index),
                        self.show_path(&name, &steps, &self.field_name(&ty, later), later),
                    );
                    let (doing, instead) = match access {
                        Access::Assign => ("assigning", "replace the whole value"),
                        _ => ("lending", "lend the whole value"),
                    };
                    self.diagnostics.push(
                        Diagnostic::error(
                            "L0233",
                            format!(
                                "{doing} `{assigned}` alone would invalidate `{dependent}`; {instead}"
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
        Ok((steps, ty))
    }

    /// A new version of the binding at `slot`: the version current now with
    /// the path replaced by `value`, rebuilt as lowering rebuilds it, under
    /// a fresh identity and equation, which every later mention is. Tracked
    /// evidence that speaks of the binding is stale from here; the binding
    /// itself, if it is tracked evidence, has just been established over
    /// the current versions. Returns the version's binder and equation.
    pub(super) fn write_back(
        &mut self,
        slot: usize,
        steps: &[Step],
        value: Term,
        access: Access,
        span: Span,
    ) -> Elab<(Binder, HypId)> {
        let name = self.names[slot].name.clone();
        let binding = self.names[slot]
            .binding
            .expect("a place written to is rooted at a mutable binding");
        let declared = self.version_type(slot);
        let current = self.names[slot].id;
        let whole = match rebuilt(Term::var(current), steps, value) {
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
        self.invalidate(
            binding,
            &Stale {
                dep: name.clone(),
                span,
                by_loop: false,
                lent: access == Access::LendMut,
            },
        );
        if let Some(tracked) = &mut self.names[slot].tracked {
            tracked.stale = None;
        }
        Ok((
            Binder {
                id: version,
                name,
                ty: declared,
                ghost: false,
            },
            equation,
        ))
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
                .filter_map(|(slot, local)| {
                    local.binding.map(|binding| EntryBinding {
                        slot,
                        binding,
                        version: local.id,
                        stale: local
                            .tracked
                            .as_ref()
                            .and_then(|tracked| tracked.stale.clone()),
                    })
                })
                .collect(),
        )
    }

    /// The versions the entry bindings have now: at the end of an arm,
    /// before its scope closes.
    pub(super) fn versions_now(&self, entry: &Entry) -> Vec<VarId> {
        entry
            .0
            .iter()
            .map(|binding| self.names[binding.slot].id)
            .collect()
    }

    /// Which of the entry bindings are stale tracked evidence now, and why.
    pub(super) fn stale_now(&self, entry: &Entry) -> Vec<Option<Stale>> {
        entry
            .0
            .iter()
            .map(|binding| {
                self.names[binding.slot]
                    .tracked
                    .as_ref()
                    .and_then(|tracked| tracked.stale.clone())
            })
            .collect()
    }

    /// What an exit of a loop ended with: a path that reaches the join.
    pub(super) fn arm_end(&self, entry: &Entry, ty: Type, never: bool) -> ArmEnd {
        ArmEnd {
            versions: self.versions_now(entry),
            stale: self.stale_now(entry),
            ty,
            never,
            leaves: false,
        }
    }

    /// The facts a block added, when it assigned an entry binding: they
    /// speak of identities that stay declared, and what the block assigned
    /// is what the code after it reads.
    pub(super) fn facts_since(&self, mark: &Mark, entry: &Entry) -> Vec<Fact> {
        if self.versions_now(entry) == entry.versions() {
            return Vec::new();
        }
        self.facts[mark.facts()..].to_vec()
    }

    /// Puts the entry versions back once an arm's scope has closed, and
    /// with them whether each tracked evidence was valid: what the arm
    /// assigned reaches the code after the branch through the join alone.
    pub(super) fn restore_versions(&mut self, entry: &Entry) {
        for binding in &entry.0 {
            let local = &mut self.names[binding.slot];
            local.id = binding.version;
            if let Some(tracked) = &mut local.tracked {
                tracked.stale = binding.stale.clone();
            }
        }
    }

    /// Whether an arm's value type mentions a version the arm made, which
    /// no other arm can be checked against.
    pub(super) fn mentions_arm_version(entry: &Entry, versions: &[VarId], ty: &Type) -> bool {
        entry
            .0
            .iter()
            .zip(versions)
            .any(|(binding, now)| *now != binding.version && type_mentions(ty, *now))
    }

    /// Joins the arms of a branch. When no arm assigned an entry binding,
    /// nothing happens and the type is `fallback`, the one the branch has
    /// today. Otherwise every assigned binding gets a version for after the
    /// branch, the value's type is stated over those versions, and the
    /// mirrored context binds the tuple's parts as lowering will. Either
    /// way, tracked evidence the branch did not assign is stale afterwards
    /// when it is stale at the end of any arm that reaches the join.
    pub(super) fn join(
        &mut self,
        entry: &Entry,
        arms: &[ArmEnd],
        fallback: Type,
        result: VarId,
        span: Span,
    ) -> Elab<(Option<Joined>, Type)> {
        // As lowering computes it: over the arms that reach the join, less
        // the evidence some such arm leaves stale, which lowering lets the
        // join leave out; that evidence is stale afterwards, and its next
        // use is where it is reported.
        let reaching: Vec<&ArmEnd> = arms.iter().filter(|arm| !arm.leaves).collect();
        let assigned: Vec<usize> = (0..entry.0.len())
            .filter(|&i| {
                reaching
                    .iter()
                    .any(|arm| arm.versions[i] != entry.0[i].version)
                    && !(matches!(self.names[entry.0[i].slot].ty, Type::Proof(_))
                        && reaching.iter().any(|arm| arm.stale[i].is_some()))
            })
            .collect();
        for (i, binding) in entry.0.iter().enumerate() {
            if assigned.contains(&i) {
                continue;
            }
            let stale = reaching.iter().find_map(|arm| arm.stale[i].clone());
            if let Some(tracked) = &mut self.names[binding.slot].tracked {
                tracked.stale = stale;
            }
        }
        if assigned.is_empty() {
            return Ok((None, fallback));
        }
        let equation = HypId::fresh();
        let value = JoinValue {
            fallback,
            bound: Some((result, equation)),
        };
        let (tuple, joins, ty) = self.join_over(
            entry,
            &assigned,
            arms,
            value,
            "before this arm ends, since the branch joins it",
            span,
        )?;
        Ok((
            Some(Joined {
                tuple,
                joins,
                equation,
            }),
            ty,
        ))
    }

    /// Joins the paths that reach the end of a branch, or leave a loop,
    /// given which entry bindings (by position) were assigned: each gets a
    /// version for afterwards, typed over the versions before it, so that
    /// tracked evidence in the tuple speaks of the joined versions of what
    /// it mentions; the value's type is stated over those versions, and the
    /// mirrored context binds the tuple's parts as lowering will, the
    /// versions and then the value under the identity and equation `value`
    /// binds it to when it has one. Tracked evidence that is assigned must be
    /// valid at the end of every path that reaches the join, since each
    /// supplies it at its own versions; afterwards it is valid. Returns the
    /// tuple's identity, the joins, and the value's type.
    pub(super) fn join_over(
        &mut self,
        entry: &Entry,
        assigned: &[usize],
        arms: &[ArmEnd],
        value: JoinValue,
        when: &str,
        span: Span,
    ) -> Elab<(VarId, Vec<Join>, Type)> {
        let JoinValue { fallback, bound } = value;
        for &i in assigned {
            let slot = entry.0[i].slot;
            if self.names[slot].tracked.is_none() {
                continue;
            }
            if let Some(stale) = arms
                .iter()
                .filter(|arm| !arm.leaves)
                .find_map(|arm| arm.stale[i].clone())
            {
                return self.stale_error(slot, &stale, "L0246", when, span);
            }
        }
        // Each joined version is typed over the ones before it: the type is
        // stated once the earlier bindings are at their joined versions.
        let mut joins: Vec<Join> = Vec::new();
        for &i in assigned {
            let slot = entry.0[i].slot;
            let join = Join {
                binding: entry.0[i].binding,
                version: Binder {
                    id: VarId::fresh(),
                    name: self.names[slot].name.clone(),
                    ty: self.version_type(slot),
                    ghost: false,
                },
                equation: HypId::fresh(),
            };
            self.names[slot].id = join.version.id;
            joins.push(join);
        }
        // An arm's value is typed over the versions it ended with; over the
        // joined versions it is the type of the whole. When the arms agree
        // only as written, the type is a snapshot of what they wrote.
        let renamed = |arm: &ArmEnd| {
            joins
                .iter()
                .zip(assigned)
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
                        "the paths that leave here produce values of different types over what they assign",
                        span,
                    );
                }
            }
        };
        let tuple = VarId::fresh();
        let declared = self.ctx.declare_with(
            tuple,
            join_type(&joins, bound.map(|(result, _)| (result, &ty))),
            false,
        );
        self.kernel(declared, span)?;
        let label = self
            .text(span)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.labels.insert(tuple, label.clone());
        let tuple_term = Term::var(tuple);
        let mut earlier: Vec<Named> = Vec::new();
        for (index, (join, &i)) in joins.iter().zip(assigned).enumerate() {
            let found = self.bind_part(
                &tuple_term,
                index,
                join.version.id,
                join.equation,
                &mut earlier,
                span,
            )?;
            let slot = entry.0[i].slot;
            self.labels
                .insert(join.version.id, join.version.name.clone());
            self.learn_from(&Term::var(join.version.id), &found);
            if let Some(tracked) = &mut self.names[slot].tracked {
                tracked.stale = None;
            }
        }
        if let Some((result, equation)) = bound {
            let found = self.bind_part(
                &tuple_term,
                joins.len(),
                result,
                equation,
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
        }
        Ok((tuple, joins, ty))
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
