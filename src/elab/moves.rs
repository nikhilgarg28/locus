//! Moves: which locals a runtime use consumes, and the refusal of a use
//! after it (O1).
//!
//! Values move, as in Rust. A type is `Copy` when it is `bool`, a machine
//! integer, a tuple of `Copy` types, a struct or enum with `Copy` in its
//! `#[derive(...)]`, or a type with no runtime form at all: `Prop`,
//! evidence, `Int`, and functions of the logic are never moved, so a proof
//! stays usable however often it is passed. Everything else moves when it
//! is used by value: passed to a call, bound by `let`, put in a tuple, a
//! struct, or a variant, returned as a block's value, taken by a `break`, or
//! assigned to a place. A field access `x.f` moves the field alone when it
//! is not `Copy` (a partial move: `x` as a whole is then gone and its other
//! fields remain), and a `let` or `match` pattern over a place moves just
//! the parts its bindings take, exactly as rustc reasons about places.
//!
//! The analysis is interleaved with elaboration, in the shape of the
//! version tracking of `mutation.rs` and `loops.rs`: each `Local` carries
//! the paths moved out of it (`Moved`), a branch snapshots that state at
//! entry, runs each arm on it, and joins the arms' states by union, since
//! a value moved on any path is gone after the join (Rust's rule); a loop
//! snapshots the state at entry and compares it at every back edge, the end
//! of the body and each `continue`, where a local declared outside the loop
//! and moved inside it without being assigned again is reported at the
//! move, "in previous iteration of loop", again as rustc does; what is
//! moved after the loop is the union over its exits. An assignment `x = e;`
//! makes `x` whole again, and `x.f = e;` its field. Nothing here is
//! trusted: the check IR is unchanged by moves (a value used twice is
//! harmless in the logic), and rustc rejects any use after a move that
//! slipped through with E0382, which is what the test hook `checked =
//! false` exposes on purpose.
//!
//! A proposition, `prop!`, `prove!`, `@(...)`, and any expression erasure
//! removes, the arguments of a call to a function with no runtime form or
//! of a proposition's constructor, mention a local without moving it: it
//! is a reading of the value, which asks that the local be live and leaves
//! it so. Mentioning a moved local there is refused as a rule of the
//! language (the claim would speak of a value the code no longer has), with
//! a model observation named as the way to keep the value: evidence obtained before
//! the move remains valid, being a value of its own.
//!
//! A lend, `&x` or `&mut x.f` as the argument of a call, reads the place
//! too: it asks that the place be whole, and leaves it so, since the
//! reference lasts for the call alone (O3). Nothing is moved out of a
//! reference parameter, `x: &T` or `x: &mut T`, or out of a field of one
//! that is not `Copy`, and a pattern over one binds only `Copy` parts:
//! Rust refuses each with E0507, and so does Locus, before rustc would.

use std::collections::HashSet;

use crate::ast::{self, ExprKind, PatternKind};
use crate::diagnostic::Diagnostic;
use crate::kernel::Type;
use crate::source::Span;
use crate::typed::{Binder, Derive, Expr, Pattern, Stmt};

use super::env::{Elab, Env};
use super::exprs::Value;

/// One move out of a local: the fields moved, by position from the local
/// (empty for the whole), where it happened, and whether it took a part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Moved {
    pub path: Vec<usize>,
    pub at: Span,
    pub partial: bool,
}

/// The state of the analysis that is not per local.
#[derive(Clone, Debug)]
pub(super) struct Moves {
    /// The test hook: `false` skips the analysis, so that the Rust printed
    /// for a program with a use after a move can be handed to rustc.
    pub checked: bool,
    /// The next local resolved is the root of a place, `x` in `x.f`, or the
    /// value a `let` or a `match` takes apart: it is not moved as a whole
    /// where it is named; the form around it says what is moved.
    place_root: bool,
    /// How many erased positions are open, besides formulas: the arguments
    /// of a call that erasure removes. A mention there is a reading.
    ghost: usize,
    /// How many lends are open: the place of a `&x` or `&mut x` argument
    /// is being elaborated. A mention there is a reading.
    lend: usize,
    /// Moves reported at a back edge of a loop, each once.
    reported: HashSet<(usize, usize)>,
}

impl Moves {
    pub fn new(checked: bool) -> Self {
        Self {
            checked,
            place_root: false,
            ghost: 0,
            lend: 0,
            reported: HashSet::new(),
        }
    }

    /// Opens an erased position that a loop with early returns elaborates;
    /// `leave_ghost` puts the depth back. A function's elaboration starts
    /// at depth zero whatever an earlier failure left (`start_item`).
    pub fn enter_ghost(&mut self) -> usize {
        let before = self.ghost;
        self.ghost += 1;
        before
    }

    pub fn leave_ghost(&mut self, before: usize) {
        self.ghost = before;
    }

    /// At the start of an item: outside any erased position or place, and
    /// with nothing reported yet, since a function of the logic may be
    /// elaborated a second time as an ordinary one with its first
    /// diagnostics dropped (`items.rs`).
    pub fn start_item(&mut self) {
        self.ghost = 0;
        self.lend = 0;
        self.place_root = false;
        self.reported.clear();
    }
}

/// The moved paths of every local in scope, in slot order: what a branch
/// or a loop finds at entry, and what an arm or an exit ends with.
#[derive(Clone, Debug)]
pub(super) struct MoveState(Vec<Vec<Moved>>);

/// A loop's part of the state: what was moved at entry, which every back
/// edge is compared with, and what each exit ended with.
#[derive(Clone, Debug)]
pub(super) struct LoopMoves {
    entry: MoveState,
    exits: Vec<MoveState>,
}

/// A local and a path of fields into it, as an expression names them.
type PlacePath = (usize, Vec<usize>);

/// The root name of `x`, `x.f`, `x.0.g`, `(x).f`, or `*self`, if it is a
/// name.
fn place_root(expr: &ast::Expr) -> Option<&ast::Name> {
    match &expr.kind {
        ExprKind::Name(name) => Some(name),
        ExprKind::Group(inner)
        | ExprKind::Member { value: inner, .. }
        | ExprKind::Index { value: inner, .. }
        | ExprKind::Unary {
            operator: ast::UnaryOp::Deref,
            expr: inner,
            ..
        } => place_root(inner),
        _ => None,
    }
}

/// Whether one path is a prefix of the other: the two places overlap, so
/// that a move of either is a move of something in the other.
fn overlap(left: &[usize], right: &[usize]) -> bool {
    left.iter().zip(right).all(|(l, r)| l == r)
}

/// `x.f.0`, for a message.
fn show_path(name: &str, path: &[usize], fields: &[Option<String>]) -> String {
    let mut shown = name.to_string();
    for (index, field) in path.iter().zip(fields) {
        shown.push('.');
        match field {
            Some(field) => shown.push_str(field),
            None => shown.push_str(&index.to_string()),
        }
    }
    shown
}

impl Env<'_> {
    // --- Types ---

    /// Whether a type is `Copy`: reusable without a move.
    pub(super) fn is_copy(&self, ty: &Type) -> bool {
        self.derives_trait(ty, Derive::Copy)
    }

    /// Whether a type has a trait of the closed list: a type with no
    /// runtime form has them all, a machine type and `bool` too, a tuple
    /// has what all its fields have, and a struct or an enum what it
    /// derived.
    pub(super) fn derives_trait(&self, ty: &Type, derive: Derive) -> bool {
        match ty {
            Type::Instance(base, _) => self.derives_trait(base, derive),
            Type::Boxed(element) | Type::Buffer(element) => {
                derive != Derive::Copy && self.derives_trait(element, derive)
            }
            Type::Bool
            | Type::U8
            | Type::Machine(_)
            | Type::Int
            | Type::Prop
            | Type::Proof(_)
            | Type::Fn(..) => true,
            Type::Tuple(fields) => fields.iter().all(|field| self.derives_trait(field, derive)),
            Type::Struct(id) => self
                .struct_by_id(*id)
                .is_some_and(|info| info.derives.contains(&derive)),
            Type::Enum(id) => self
                .enum_by_id(*id)
                .is_some_and(|info| info.derives.contains(&derive)),
        }
    }

    /// The first piece of logic-only data in a type, at any depth, as a
    /// path for a message: `Proved` markers would compare equal and lie,
    /// so a type holding one has no equality at runtime.
    pub(super) fn logic_only_data(&self, ty: &Type) -> Option<String> {
        match ty {
            Type::Instance(base, _) => self.logic_only_data(base),
            Type::Boxed(element) | Type::Buffer(element) => self
                .logic_only_data(element)
                .map(|inner| format!("[]{inner}")),
            Type::Bool | Type::U8 | Type::Machine(_) => None,
            Type::Int | Type::Prop | Type::Proof(_) | Type::Fn(..) => Some(String::new()),
            Type::Tuple(fields) => fields.iter().enumerate().find_map(|(index, field)| {
                self.logic_only_data(field)
                    .map(|inner| format!(".{index}{inner}"))
            }),
            Type::Struct(id) => {
                let info = self.struct_by_id(*id)?;
                info.fields.iter().find_map(|field| {
                    self.logic_only_data(&field.ty)
                        .map(|inner| format!(".{}{inner}", field.name))
                })
            }
            Type::Enum(id) => {
                let info = self.enum_by_id(*id)?;
                info.variants.iter().find_map(|variant| {
                    variant
                        .payload
                        .iter()
                        .enumerate()
                        .find_map(|(index, field)| {
                            self.logic_only_data(&field.ty).map(|inner| {
                                if variant.named {
                                    format!("::{}.{}{inner}", variant.name, field.name)
                                } else {
                                    format!("::{}.{index}{inner}", variant.name)
                                }
                            })
                        })
                })
            }
        }
    }

    // --- Places ---

    /// Whether a mention here reads a value rather than consuming it: in a
    /// formula, in an argument erasure removes, or in a lent place. It is
    /// also where a `Ghost<T>` value may be named (`exprs.rs`).
    pub(super) fn reading(&self) -> bool {
        self.formula.is_some() || self.moves.ghost > 0 || self.moves.lend > 0
    }

    /// Elaborates `inside` as a lent place, the operand of `&` or `&mut`
    /// in an argument: a local named there is read, not moved.
    pub(super) fn lending<T>(&mut self, inside: impl FnOnce(&mut Self) -> T) -> T {
        self.moves.lend += 1;
        let result = inside(self);
        self.moves.lend -= 1;
        result
    }

    /// Whether the local is a reference parameter, or a version of one:
    /// nothing is moved out of it.
    fn behind_reference(&self, slot: usize) -> bool {
        let local = &self.names[slot];
        self.borrowed.contains(&local.binding.unwrap_or(local.id))
    }

    /// `L0265`: a move out of a reference parameter, or out of a field of
    /// one, which Rust refuses with E0507. `path` is the moved part.
    fn move_out_of_reference<T>(&mut self, slot: usize, path: &[usize], span: Span) -> Elab<T> {
        let local = &self.names[slot];
        let name = local.name.clone();
        let mutable = local.binding.is_some();
        let fields = self.field_names(slot, path);
        let shown = if path.is_empty() {
            format!("*{name}")
        } else {
            show_path(&name, path, &fields)
        };
        let kind = if mutable { "mutable" } else { "shared" };
        let ty = self.show_type(&self.names[slot].ty.clone());
        self.diagnostics.push(
            Diagnostic::error(
                "L0265",
                format!("cannot move out of `{shown}` which is behind a {kind} reference (E0507)"),
                span,
            )
            .note(format!(
                "`{name}` is a reference parameter, `{name}: &{}{ty}`, and the value behind it belongs to the caller; a part that is `Copy` can be read, and the whole can be lent on with `&{}{name}`",
                if mutable { "mut " } else { "" },
                if mutable { "mut " } else { "" }
            )),
        );
        Err(())
    }

    /// The names of the fields along a path into the local, for a message.
    fn field_names(&self, slot: usize, path: &[usize]) -> Vec<Option<String>> {
        let mut ty = self.names[slot].ty.clone();
        let mut names = Vec::new();
        for &index in path {
            let name = match ty.nominal() {
                Type::Struct(id) => self
                    .struct_by_id(*id)
                    .and_then(|info| info.fields.get(index).map(|field| field.name.clone())),
                _ => None,
            };
            ty = match ty.nominal() {
                Type::Struct(id) => self
                    .struct_by_id(*id)
                    .and_then(|info| info.fields.get(index).map(|field| field.ty.clone()))
                    .unwrap_or(Type::Tuple(Vec::new())),
                Type::Tuple(fields) => fields
                    .get(index)
                    .cloned()
                    .unwrap_or(Type::Tuple(Vec::new())),
                _ => Type::Tuple(Vec::new()),
            };
            names.push(name);
        }
        names
    }

    /// Elaborates `inside` as an erased position: the arguments of a call
    /// with no runtime form. A local named there is read, not moved.
    pub(super) fn ghost<T>(&mut self, inside: impl FnOnce(&mut Self) -> T) -> T {
        self.moves.ghost += 1;
        let result = inside(self);
        self.moves.ghost -= 1;
        result
    }

    /// Type resolution of a non-place method receiver must decide its mode
    /// before evaluating ownership. A discarded probe disables this flag, so
    /// nested receivers do not recursively probe again.
    pub(super) fn needs_receiver_mode(&self) -> bool {
        self.moves.checked && !self.total
    }

    /// A retained runtime call consumes its by-value arguments, even when
    /// the call occurs inside an erased observation. Only the outer logical
    /// operation reads without moving; nested runtime evaluation is ordinary.
    pub(super) fn runtime_arguments<T>(&mut self, inside: impl FnOnce(&mut Self) -> T) -> T {
        let ghost = std::mem::replace(&mut self.moves.ghost, 0);
        let lend = std::mem::replace(&mut self.moves.lend, 0);
        let result = inside(self);
        self.moves.ghost = ghost;
        self.moves.lend = lend;
        result
    }

    /// Before elaborating an expression that is a place rooted at a local,
    /// `x.f` or the value of a `let` or `match`: the local is not moved
    /// where it is named; `place_used` or the pattern says what is.
    /// Returns whether this is the outermost form of the place: `x.f` in
    /// `x.f.g` is elaborated with the mark already set, and is not a use of
    /// its own.
    pub(super) fn mark_place_root(&mut self, expr: &ast::Expr) -> bool {
        let outermost = !self.moves.place_root;
        if self.moves.checked
            && let Some(root) = place_root(expr)
            && self.lookup(&root.text).is_some()
        {
            self.moves.place_root = true;
        }
        outermost
    }

    /// The local and path an elaborated place names, if it is one:
    /// `Field` over `Field` over a `Var` that is a local in scope.
    pub(super) fn place_of(&self, expr: &Expr) -> Option<PlacePath> {
        let mut path = Vec::new();
        let mut expr = expr;
        loop {
            match expr {
                Expr::Field { target, index, .. } => {
                    path.push(*index);
                    expr = target;
                }
                Expr::Var { id, .. } => {
                    let slot = self.names.iter().rposition(|local| local.id == *id)?;
                    path.reverse();
                    return Some((slot, path));
                }
                _ => return None,
            }
        }
    }

    /// After `x.f` or `x.0` was elaborated: the field is read where a
    /// mention reads, copied when its type is `Copy`, and moved otherwise.
    pub(super) fn place_used(
        &mut self,
        value: Elab<Value>,
        span: Span,
        outermost: bool,
    ) -> Elab<Value> {
        if !outermost {
            return value;
        }
        self.moves.place_root = false;
        let value = value?;
        if self.moves.checked
            && let Some((slot, path)) = self.place_of(&value.expr)
        {
            self.use_place(slot, path, &value.ty, span);
        }
        Ok(value)
    }

    /// After the value of a `let` or a `match` was elaborated: the place it
    /// names, if any, read here (a moved value cannot be taken apart, even
    /// by `_`); the pattern then moves the parts it binds.
    pub(super) fn place_taken(&mut self, value: &Value, span: Span) -> Option<PlacePath> {
        self.moves.place_root = false;
        if !self.moves.checked {
            return None;
        }
        let (slot, path) = self.place_of(&value.expr)?;
        self.read_place(slot, &path, span);
        Some((slot, path))
    }

    /// A local named as a value, `x`: read where a mention reads, copied
    /// when `Copy`, moved otherwise, unless it is the root of a place.
    pub(super) fn use_local(&mut self, slot: usize, span: Span) {
        if !self.moves.checked {
            return;
        }
        if std::mem::take(&mut self.moves.place_root) {
            return;
        }
        let ty = self.names[slot].ty.clone();
        self.use_place(slot, Vec::new(), &ty, span);
    }

    pub(super) fn consume_value_place(&mut self, value: &Value, span: Span) {
        if let Some((slot, path)) = self.place_of(&value.expr) {
            self.use_place(slot, path, &value.ty, span);
        }
    }

    fn shared_place(&self, slot: usize, path: &[usize]) -> bool {
        let local = &self.names[slot];
        let mut ty = local.ty.clone();
        let mut expr = Expr::Var {
            id: local.id,
            name: local.name.clone(),
            ty: ty.clone(),
        };
        for index in path {
            ty = match ty.nominal() {
                Type::Tuple(fields) => fields.get(*index).cloned(),
                Type::Struct(id) => self
                    .struct_by_id(*id)
                    .and_then(|s| s.fields.get(*index).map(|f| f.ty.clone())),
                _ => None,
            }
            .unwrap_or(Type::Prop);
            expr = Expr::Field {
                target: Box::new(expr),
                index: *index,
                name: None,
                ty: ty.clone(),
            };
        }
        matches!(
            self.session.expression_layout(&expr),
            crate::typed::ErasureLayout::Shared { .. }
        )
    }

    fn use_place(&mut self, slot: usize, path: Vec<usize>, ty: &Type, span: Span) {
        if self.reading() {
            self.read_place(slot, &path, span);
            return;
        }
        if !self.read_place(slot, &path, span) || self.is_copy(ty) || self.shared_place(slot, &path)
        {
            return;
        }
        if self.behind_reference(slot) {
            let _: Elab<()> = self.move_out_of_reference(slot, &path, span);
            return;
        }
        let partial = !path.is_empty();
        self.names[slot].moved.push(Moved {
            path,
            at: span,
            partial,
        });
    }

    /// Whether a place is whole: nothing in it, and nothing it is in, has
    /// been moved. Otherwise the use is reported: after a move, `L0240`,
    /// and in a proposition, `L0241`.
    fn read_place(&mut self, slot: usize, path: &[usize], span: Span) -> bool {
        let Some(moved) = self.names[slot]
            .moved
            .iter()
            .find(|moved| overlap(&moved.path, path))
            .cloned()
        else {
            return true;
        };
        let name = self.names[slot].name.clone();
        if self.moves.lend > 0 {
            let mut diagnostic =
                Diagnostic::error("L0240", format!("borrow of moved value: `{name}`"), span)
                    .label(moved.at, "value moved here");
            diagnostic.labels[0].message = "value borrowed here after move".into();
            self.diagnostics.push(diagnostic.note(format!(
                "a lend, `&{name}` or `&mut {name}`, reads the value, which the code no longer has; a `let mut` is whole again once assigned"
            )));
            return false;
        }
        if self.reading() {
            let line = self
                .source
                .line_column(moved.at.start)
                .map_or(0, |(line, _)| line);
            self.diagnostics.push(
                Diagnostic::error(
                    "L0241",
                    format!(
                        "`{name}` was moved at line {line} and cannot be mentioned in a proposition afterwards; observe `{name}` with a model cast before the move"
                    ),
                    span,
                )
                .label(moved.at, "value moved here")
                .note("a proposition mentions a value the code still has; evidence obtained before the move stays valid, being a value of its own"),
            );
            return false;
        }
        let (message, label) = if moved.partial {
            (
                format!("use of partially moved value: `{name}`"),
                "value partially moved here",
            )
        } else {
            (format!("use of moved value: `{name}`"), "value moved here")
        };
        let ty = self.show_type(&self.names[slot].ty.clone());
        let mut diagnostic = Diagnostic::error("L0240", message, span).label(moved.at, label);
        diagnostic.labels[0].message = "value used here after move".into();
        self.diagnostics.push(diagnostic.note(format!(
            "move occurs because `{name}` has type `{ty}`, which does not implement the `Copy` trait; a struct or enum is `Copy` with `#[derive(Clone, Copy)]`, and a `let mut` is whole again once assigned"
        )));
        false
    }

    // --- Patterns ---

    /// The parts of a place a `let` pattern binds: each name bound to a
    /// value that is not `Copy` moves that part, and `_` moves nothing.
    /// `span` is the value's, for a name that takes the whole.
    pub(super) fn move_by_pattern(
        &mut self,
        place: Option<&PlacePath>,
        pattern: &ast::Pattern,
        typed: &Pattern,
        span: Span,
    ) {
        if self.reading() {
            return;
        }
        let Some((slot, path)) = place else {
            return;
        };
        self.move_parts(*slot, path.clone(), pattern, typed, span);
    }

    fn move_parts(
        &mut self,
        slot: usize,
        path: Vec<usize>,
        pattern: &ast::Pattern,
        typed: &Pattern,
        span: Span,
    ) {
        match (&pattern.kind, typed) {
            (PatternKind::Group(inner), _) => self.move_parts(slot, path, inner, typed, span),
            (PatternKind::Name { .. }, Pattern::Bind { binder, .. }) => {
                if !self.is_copy(&binder.ty)
                    && !matches!(
                        self.session.binding_layout(binder.id),
                        crate::typed::ErasureLayout::Shared { .. }
                    )
                {
                    // Matching through a reference binds `Copy` parts only.
                    if self.behind_reference(slot) {
                        let _: Elab<()> = self.move_out_of_reference(slot, &path, span);
                        return;
                    }
                    let partial = !path.is_empty();
                    self.names[slot].moved.push(Moved {
                        path,
                        at: span,
                        partial,
                    });
                }
            }
            (PatternKind::Tuple(parts), Pattern::Tuple(typed_parts)) => {
                for (index, (part, typed_part)) in parts.iter().zip(typed_parts).enumerate() {
                    let mut path = path.clone();
                    path.push(index);
                    self.move_parts(slot, path, part, typed_part, part.span);
                }
            }
            _ => {}
        }
    }

    /// The parts of a matched place an arm's pattern binds: a name bound
    /// to a payload field that is not `Copy` moves the value matched (a
    /// partial move, in rustc's words), in that arm alone.
    pub(super) fn move_by_arm(
        &mut self,
        place: Option<&PlacePath>,
        payload: &[Binder],
        names: &[Option<&ast::Name>],
    ) {
        if self.reading() {
            return;
        }
        let Some((slot, path)) = place else {
            return;
        };
        for (binder, name) in payload.iter().zip(names) {
            if let Some(name) = name
                && !self.is_copy(&binder.ty)
            {
                if self.behind_reference(*slot) {
                    let _: Elab<()> = self.move_out_of_reference(*slot, path, name.span);
                    return;
                }
                self.names[*slot].moved.push(Moved {
                    path: path.clone(),
                    at: name.span,
                    partial: true,
                });
                return;
            }
        }
    }

    // --- Assignment ---

    /// After `place = value;`, whose place is at `span`: the place is
    /// whole again, its moved parts with it. A part of a value that was
    /// moved as a whole cannot be assigned on its own, as rustc says.
    pub(super) fn assigned(&mut self, stmt: &Stmt, span: Span) {
        let Stmt::Assign { place, .. } = stmt else {
            return;
        };
        if !self.moves.checked {
            return;
        }
        let Some(slot) = self
            .names
            .iter()
            .rposition(|local| local.binding == Some(place.binding))
        else {
            return;
        };
        let path: Vec<usize> = place.path.iter().map(|step| step.index).collect();
        let outer = self.names[slot]
            .moved
            .iter()
            .find(|moved| moved.path.len() < path.len() && overlap(&moved.path, &path))
            .cloned();
        if let Some(moved) = outer {
            let fields: Vec<Option<String>> =
                place.path.iter().map(|step| step.name.clone()).collect();
            let whole = show_path(&place.name, &moved.path, &fields);
            let mut diagnostic = Diagnostic::error(
                "L0240",
                format!("assign to part of moved value: `{whole}`"),
                span,
            )
            .label(moved.at, "value moved here");
            diagnostic.labels[0].message = "value partially assigned here after move".into();
            self.diagnostics.push(diagnostic.note(format!(
                "assign the whole of `{whole}` to make it usable again"
            )));
        }
        self.names[slot]
            .moved
            .retain(|moved| !(moved.path.len() >= path.len() && overlap(&moved.path, &path)));
    }

    // --- Branches ---

    /// What is moved now, of every local in scope.
    pub(super) fn moves_now(&self) -> MoveState {
        MoveState(self.names.iter().map(|local| local.moved.clone()).collect())
    }

    /// Puts an entry state back, for the next arm.
    pub(super) fn restore_moves(&mut self, entry: &MoveState) {
        for (local, moved) in self.names.iter_mut().zip(&entry.0) {
            local.moved = moved.clone();
        }
    }

    /// After the arms of a branch: what any arm that reaches the join moved
    /// is moved after it, and what every such arm assigned again is whole.
    /// An arm that transfers control is left out; when none reaches the
    /// join, nothing after it runs and the entry state stands.
    pub(super) fn join_moves(&mut self, entry: &MoveState, arms: &[(MoveState, bool)]) {
        let live: Vec<&MoveState> = arms
            .iter()
            .filter(|(_, never)| !never)
            .map(|(arm, _)| arm)
            .collect();
        for (slot, at_entry) in entry.0.iter().enumerate() {
            let mut joined = Vec::new();
            for arm in &live {
                for moved in arm.0.get(slot).into_iter().flatten() {
                    if !joined
                        .iter()
                        .any(|earlier: &Moved| earlier.path == moved.path)
                    {
                        joined.push(moved.clone());
                    }
                }
            }
            if live.is_empty() {
                joined = at_entry.clone();
            }
            if let Some(local) = self.names.get_mut(slot) {
                local.moved = joined;
            }
        }
    }

    // --- Loops ---

    /// The state a loop starts from.
    pub(super) fn loop_moves(&self) -> LoopMoves {
        LoopMoves {
            entry: self.moves_now(),
            exits: Vec::new(),
        }
    }

    /// A `break`, or the exit of a `while` when its condition fails: what
    /// is moved here is moved after the loop.
    pub(super) fn loop_exit(&mut self) {
        let state = self.moves_now();
        if let Some(target) = self.loops.last_mut() {
            target.moves.exits.push(state);
        }
    }

    /// A back edge: the end of the body, or a `continue`. A local declared
    /// outside the loop and moved since its entry would be found moved by
    /// the next pass, so the move is reported, once, where it stands.
    pub(super) fn back_edge(&mut self) {
        let Some(target) = self.loops.last() else {
            return;
        };
        let entry = &target.moves.entry;
        let mut found = Vec::new();
        for (slot, at_entry) in entry.0.iter().enumerate() {
            let Some(local) = self.names.get(slot) else {
                break;
            };
            for moved in &local.moved {
                if !at_entry.iter().any(|earlier| earlier.path == moved.path) {
                    found.push((local.name.clone(), local.ty.clone(), moved.clone()));
                }
            }
        }
        for (name, ty, moved) in found {
            if !self.moves.reported.insert((moved.at.start, moved.at.end)) {
                continue;
            }
            let ty = self.show_type(&ty);
            let mut diagnostic =
                Diagnostic::error("L0240", format!("use of moved value: `{name}`"), moved.at);
            diagnostic.labels[0].message = "value moved here, in previous iteration of loop".into();
            self.diagnostics.push(diagnostic.note(format!(
                "move occurs because `{name}` has type `{ty}`, which does not implement the `Copy` trait; assign `{name}` again before the end of the body, or declare it inside the loop"
            )));
        }
    }

    /// After a loop: what its exits moved, joined as a branch's arms are.
    /// The exits are each `break`, the failing condition of a `while`, and
    /// the empty range of a `for`, which is the entry state.
    pub(super) fn leave_loop_moves(&mut self, moves: &LoopMoves) {
        let arms: Vec<(MoveState, bool)> = moves
            .exits
            .iter()
            .map(|exit| (exit.clone(), false))
            .collect();
        self.join_moves(&moves.entry, &arms);
    }
}
