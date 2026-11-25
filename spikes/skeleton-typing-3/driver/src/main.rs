//! The spike's driver. The call listing is adapted (copied, not depended
//! on) from `spikes/marker-lowering/stub-self/oracle`.
//!
//! Cargo runs it in place of rustc (`RUSTC_WORKSPACE_WRAPPER`). For Locus's
//! library and macros it is rustc. For every other crate of the workspace,
//! once rustc has checked it:
//!
//! - With `LOCUS_CHECK_UNTYPED=1`, it reports as an error every use of a
//!   local whose type holds the library's untyped marker, unless the use is
//!   inside the expansion of one of the library's macros, or is the target
//!   of a ghost statement. Such a local is a ghost local whose initializer
//!   had no typing skeleton, which lowering guessed that only logical code
//!   mentions.
//! - With `LOCUS_CALLS_DIR=dir`, it writes `dir/<crate>.calls`: for each
//!   call, method call, and use of an associated constant that the source
//!   wrote, the item rustc resolved it to, the implementation that item
//!   comes from where its type arguments say, and the type arguments. Two
//!   expansions of one source correspond only if their listings agree.
//! - With `LOCUS_MODELS_DIR=dir` (round 3), it writes `dir/<crate>.models`:
//!   every implementation of the library's `Model` that this crate can see,
//!   its own and those of the crates it depends on, with the crate that
//!   holds it, the type, and where it comes from: Locus's library, the
//!   expansion of `locus!`, or a hand. `Model` is an `unsafe trait`, so a
//!   hand-written one says `unsafe impl`; this listing is what tells it
//!   from lowering's own.
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_driver::Compilation;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::Span;

struct Passive;
impl rustc_driver::Callbacks for Passive {}

struct Checks {
    untyped: bool,
    calls: Option<String>,
    models: Option<String>,
}

impl rustc_driver::Callbacks for Checks {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        if self.untyped {
            untyped_uses(tcx);
        }
        if let Some(out) = &self.models {
            std::fs::write(out, model_impls(tcx).join("\n") + "\n").expect("write the models");
        }
        if let Some(out) = &self.calls {
            let mut lines = Vec::new();
            for owner in tcx.hir_body_owners() {
                let mut lister = Lister {
                    tcx,
                    owner: owner.to_def_id(),
                    typeck: tcx.typeck(owner),
                    lines: &mut lines,
                };
                rustc_hir::intravisit::Visitor::visit_body(
                    &mut lister,
                    tcx.hir_body_owned_by(owner),
                );
            }
            lines.sort();
            lines.dedup();
            std::fs::write(out, lines.join("\n") + "\n").expect("write the listing");
        }
        Compilation::Continue
    }
}

// ------------------------------------------------- implementations of `Model`

/// The macro `locus!` of Locus's macro crate, if `span` was written by its
/// expansion, directly or through other macros; and otherwise the macro that
/// wrote it, if one did.
fn written_by(tcx: TyCtxt<'_>, span: Span) -> (bool, Option<String>) {
    let mut span = span;
    let mut other = None;
    while span.from_expansion() {
        let data = span.ctxt().outer_expn_data();
        if let Some(d) = data.macro_def_id {
            if tcx.crate_name(d.krate).as_str() == "locus_macros" && tcx.item_name(d).as_str() == "locus" {
                return (true, None);
            }
            other.get_or_insert_with(|| tcx.def_path_str(d));
        }
        span = data.call_site;
    }
    (false, other)
}

/// One line for each implementation of `locus::Model` that this crate sees.
fn model_impls(tcx: TyCtxt<'_>) -> Vec<String> {
    let Some(krate) = tcx
        .crates(())
        .iter()
        .find(|c| tcx.crate_name(**c).as_str() == "locus")
    else {
        return Vec::new();
    };
    let Some(model) = tcx
        .module_children(krate.as_def_id())
        .iter()
        .find(|c| c.ident.name.as_str() == "Model" && matches!(c.res, Res::Def(DefKind::Trait, _)))
        .and_then(|c| c.res.opt_def_id())
    else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for id in tcx.all_impls(model) {
        let holder = tcx.crate_name(id.krate);
        let ty = tcx.type_of(id).skip_binder();
        let ty = rustc_middle::ty::print::with_no_trimmed_paths!(format!("{ty}"));
        let origin = if holder.as_str() == "locus" {
            "Locus's library".to_string()
        } else {
            match written_by(tcx, tcx.def_span(id)) {
                (true, _) => "lowering: the expansion of `locus!`".to_string(),
                (false, Some(m)) => format!("WRITTEN BY HAND, through the macro `{m}`"),
                (false, None) => "WRITTEN BY HAND".to_string(),
            }
        };
        lines.push(format!("{holder} | {ty} | {origin}"));
    }
    lines.sort();
    lines
}

// ------------------------------------------------------ untyped ghost locals

/// The crates of Locus's library and macros.
fn library(tcx: TyCtxt<'_>, id: DefId) -> bool {
    matches!(tcx.crate_name(id.krate).as_str(), "locus" | "locus_macros")
}

/// Whether `span` was written by a macro of the library, directly or
/// through other macros.
fn from_library_macro(tcx: TyCtxt<'_>, span: Span) -> bool {
    let mut span = span;
    while span.from_expansion() {
        let data = span.ctxt().outer_expn_data();
        if data.macro_def_id.is_some_and(|d| library(tcx, d)) {
            return true;
        }
        span = data.call_site;
    }
    false
}

fn untyped_uses(tcx: TyCtxt<'_>) {
    // The library's untyped marker.
    let Some(krate) = tcx
        .crates(())
        .iter()
        .find(|c| tcx.crate_name(**c).as_str() == "locus")
    else {
        return;
    };
    let Some(marker) = tcx
        .module_children(krate.as_def_id())
        .iter()
        .find(|c| c.ident.name.as_str() == "Untyped")
        .and_then(|c| c.res.opt_def_id())
    else {
        return;
    };
    for owner in tcx.hir_body_owners() {
        let mut uses = Uses {
            tcx,
            typeck: tcx.typeck(owner),
            marker,
        };
        rustc_hir::intravisit::Visitor::visit_body(&mut uses, tcx.hir_body_owned_by(owner));
    }
}

struct Uses<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx ty::TypeckResults<'tcx>,
    marker: DefId,
}

impl<'tcx> Uses<'tcx> {
    fn holds_marker(&self, ty: ty::Ty<'tcx>) -> bool {
        ty.walk().any(|arg| {
            arg.as_type()
                .is_some_and(|t| matches!(t.kind(), ty::Adt(def, _) if def.did() == self.marker))
        })
    }

    /// Whether the use is logical code: inside what a macro of the library
    /// wrote, or the target of a ghost statement, `x = logic! { .. };`.
    fn logical(&self, expr: &rustc_hir::Expr<'tcx>) -> bool {
        if from_library_macro(self.tcx, expr.span) {
            return true;
        }
        for (id, node) in self.tcx.hir_parent_iter(expr.hir_id) {
            match node {
                rustc_hir::Node::Expr(parent) => {
                    if from_library_macro(self.tcx, parent.span) {
                        return true;
                    }
                    if let rustc_hir::ExprKind::Assign(lhs, rhs, _) = parent.kind {
                        if lhs.hir_id == expr.hir_id && from_library_macro(self.tcx, rhs.span) {
                            return true;
                        }
                    }
                }
                rustc_hir::Node::Stmt(_) | rustc_hir::Node::Block(_) | rustc_hir::Node::LetStmt(_) => {
                    if from_library_macro(self.tcx, self.tcx.hir_span(id)) {
                        return true;
                    }
                }
                _ => return false,
            }
        }
        false
    }
}

impl<'tcx> rustc_hir::intravisit::Visitor<'tcx> for Uses<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        if let rustc_hir::ExprKind::Path(rustc_hir::QPath::Resolved(None, path)) = expr.kind {
            if let Res::Local(local) = path.res {
                let ty = self.typeck.node_type(local);
                // A local that a macro wrote, such as the arguments of
                // `format!`, is not one to name: the use it was made from is.
                let written = !self.tcx.hir_span(local).from_expansion();
                // `pair.0` uses the component alone.
                let mut whole = expr;
                for (_, node) in self.tcx.hir_parent_iter(expr.hir_id) {
                    match node {
                        rustc_hir::Node::Expr(parent)
                            if matches!(parent.kind, rustc_hir::ExprKind::Field(base, _) if base.hir_id == whole.hir_id) =>
                        {
                            whole = parent;
                        }
                        _ => break,
                    }
                }
                let used = self.typeck.expr_ty(whole);
                if written && self.holds_marker(used) && !self.logical(whole) {
                    let name = self.tcx.hir_name(local);
                    self.tcx.dcx().span_err(
                        whole.span,
                        format!(
                            "locus: the ghost local `{name}` has no logical type for rustc, and it is used here outside logical code: its type is `{ty}`. Write the type of the `logic!` that it holds"
                        ),
                    );
                }
            }
        }
        rustc_hir::intravisit::walk_expr(self, expr);
    }
}

// ------------------------------------------------------------ the listing

struct Lister<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    owner: DefId,
    typeck: &'tcx ty::TypeckResults<'tcx>,
    lines: &'a mut Vec<String>,
}

impl<'tcx> Lister<'_, 'tcx> {
    /// Whether the source text at `span` is the name of the item, or the
    /// name of the logic function whose stub the item is.
    fn written(&self, span: Span, id: DefId) -> bool {
        let name = self.tcx.item_name(id);
        let name = name.as_str();
        self.tcx
            .sess
            .source_map()
            .span_to_snippet(span)
            .is_ok_and(|text| text == name || name.strip_prefix("__logic_") == Some(text.as_str()))
    }

    /// One line: where, the item, the implementation, the type arguments.
    fn record(&mut self, span: Span, id: DefId, args: ty::GenericArgsRef<'tcx>) {
        let tcx = self.tcx;
        // What the source wrote, and not what Locus's library is, nor what
        // lowering wrote: an item it generated carries the span of a source
        // token that is not its name.
        if span.from_expansion()
            || tcx.crate_name(id.krate).as_str() == "locus"
            || !self.written(span, id)
        {
            return;
        }
        let at = tcx.sess.source_map().lookup_char_pos(span.lo());
        let mut text = tcx.def_path_str(id);
        let mut resolved = id;
        // The implementation the item comes from, where the arguments say.
        if !ty::TypeVisitableExt::has_param(&args) && tcx.trait_of_assoc(id).is_some() {
            let env = ty::TypingEnv::fully_monomorphized();
            let args = tcx.erase_and_anonymize_regions(args);
            if let Ok(Some(instance)) = ty::Instance::try_resolve(tcx, env, id, args) {
                if instance.def_id() != id {
                    resolved = instance.def_id();
                    text.push_str(&format!(" -> {}", tcx.def_path_str(resolved)));
                }
            }
        }
        if resolved.is_local()
            && !tcx
                .def_ident_span(resolved)
                .is_some_and(|s| self.written(s, resolved))
        {
            return;
        }
        // What two expansions are compared by: where the definition is
        // written, for one of this crate, and its path otherwise.
        let key = if resolved.is_local() {
            let d = tcx
                .sess
                .source_map()
                .lookup_char_pos(tcx.def_span(resolved).lo());
            format!("the definition at {}:{}", d.line, d.col.0 + 1)
        } else {
            tcx.def_path_str(resolved)
        };
        let line = format!(
            "{}:{:03}:{:03} => {key} | {text} with {args:?}",
            tcx.def_path_str(self.owner),
            at.line,
            at.col.0 + 1
        );
        self.lines.push(line.replace("locus::", ""));
    }
}

impl<'tcx> rustc_hir::intravisit::Visitor<'tcx> for Lister<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        use rustc_hir::ExprKind;
        match expr.kind {
            ExprKind::MethodCall(segment, ..) => {
                if let Some(id) = self.typeck.type_dependent_def_id(expr.hir_id) {
                    self.record(segment.ident.span, id, self.typeck.node_args(expr.hir_id));
                }
            }
            ExprKind::Path(ref qpath) => {
                let res = self.typeck.qpath_res(qpath, expr.hir_id);
                let name = match qpath {
                    rustc_hir::QPath::Resolved(_, path) => {
                        path.segments.last().map(|s| s.ident.span)
                    }
                    rustc_hir::QPath::TypeRelative(_, segment) => Some(segment.ident.span),
                };
                if let (
                    Res::Def(DefKind::Fn | DefKind::AssocFn | DefKind::AssocConst { .. }, id),
                    Some(name),
                ) = (res, name)
                {
                    self.record(name, id, self.typeck.node_args(expr.hir_id));
                }
            }
            _ => {}
        }
        rustc_hir::intravisit::walk_expr(self, expr);
    }

    /// The closures of typing skeletons are bodies of their own.
    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

fn main() {
    // `driver /path/to/rustc ARGS..`, as Cargo runs a wrapper.
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a.starts_with("--sysroot")) {
        if let Ok(sysroot) = std::env::var("LOCUS_SYSROOT") {
            args.push("--sysroot".to_string());
            args.push(sysroot);
        }
    }
    let name = args
        .iter()
        .position(|a| a == "--crate-name")
        .and_then(|i| args.get(i + 1).cloned());
    let program = name
        .as_deref()
        .is_some_and(|n| n != "locus" && n != "locus_macros" && n != "___");
    let untyped = std::env::var("LOCUS_CHECK_UNTYPED").is_ok();
    let calls = std::env::var("LOCUS_CALLS_DIR")
        .ok()
        .zip(name.clone())
        .map(|(dir, name)| format!("{dir}/{name}.calls"));
    let models = std::env::var("LOCUS_MODELS_DIR")
        .ok()
        .zip(name.clone())
        .map(|(dir, name)| format!("{dir}/{name}.models"));
    if program && (untyped || calls.is_some() || models.is_some()) {
        rustc_driver::run_compiler(&args, &mut Checks { untyped, calls, models });
    } else {
        rustc_driver::run_compiler(&args, &mut Passive);
    }
}
