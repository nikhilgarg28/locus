//! Spike: a rustc driver that reports the facts Locus needs from a checking
//! program, then lets rustc continue so that its own checks (including the
//! borrow checker) run as usual.
//!
//! Run as Cargo's RUSTC_WORKSPACE_WRAPPER. For the crate named by
//! LOCUS_SPIKE_CRATE, the crate root LOCUS_SPIKE_ROOT_FROM is replaced by
//! LOCUS_SPIKE_ROOT_TO, and facts are written to LOCUS_SPIKE_FACTS as JSON
//! lines:
//!
//! - observations: each `__locus::observe(&place)`, with the place's type
//! - resolutions: each `__locus::resolve(path)`, with the item it names
//! - body: for the function named by LOCUS_SPIKE_BODY, each expression of its
//!   typed body (THIR) with its type
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use std::fmt::Write as _;

use rustc_driver::Compilation;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind};
use rustc_middle::ty::{TyCtxt, TypeckResults};

struct Facts {
    out: std::path::PathBuf,
}

impl rustc_driver::Callbacks for Facts {
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let mut lines = String::new();
        for def_id in tcx.hir_body_owners() {
            let typeck = tcx.typeck(def_id);
            let body = tcx.hir_body_owned_by(def_id);
            let mut finder = Finder {
                tcx,
                typeck,
                lines: &mut lines,
            };
            finder.visit_body(body);
            if std::env::var("LOCUS_SPIKE_BODY").is_ok_and(|f| f == tcx.def_path_str(def_id)) {
                dump_thir(tcx, def_id, &mut lines);
            }
        }
        std::fs::write(&self.out, lines).expect("write facts");
        // Continue: rustc's own analysis, including borrow checking, still runs.
        Compilation::Continue
    }
}

struct Finder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    lines: &'a mut String,
}

impl<'a, 'tcx> Visitor<'tcx> for Finder<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Call(callee, args) = expr.kind
            && let ExprKind::Path(ref qpath) = callee.kind
            && let Res::Def(_, def_id) = self.typeck.qpath_res(qpath, callee.hir_id)
            && let [arg] = args
        {
            let at = self
                .tcx
                .sess
                .source_map()
                .span_to_diagnostic_string(arg.span);
            match self.tcx.def_path_str(def_id).as_str() {
                "__locus::observe" => {
                    let place = match arg.kind {
                        ExprKind::AddrOf(_, _, inner) => inner,
                        _ => arg,
                    };
                    let ty = self.typeck.expr_ty(place);
                    let _ = writeln!(
                        self.lines,
                        r#"{{"kind":"observation","at":"{at}","type":"{ty}"}}"#
                    );
                }
                "__locus::resolve" => {
                    if let ExprKind::Path(ref target) = arg.kind
                        && let Res::Def(kind, id) = self.typeck.qpath_res(target, arg.hir_id)
                    {
                        let _ = writeln!(
                            self.lines,
                            r#"{{"kind":"resolution","at":"{at}","item":"{}","def_kind":"{kind:?}"}}"#,
                            self.tcx.def_path_str(id)
                        );
                    }
                }
                _ => {}
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

/// One line per THIR expression: its kind, without its fields, and its type.
fn dump_thir(tcx: TyCtxt<'_>, def_id: rustc_span::def_id::LocalDefId, lines: &mut String) {
    let Ok((thir, _root)) = tcx.thir_body(def_id) else { return };
    let thir = thir.borrow();
    for expr in thir.exprs.iter() {
        let kind = format!("{:?}", expr.kind);
        let kind = kind.split(['(', ' ', '{']).next().unwrap_or("");
        let detail = match &expr.kind {
            rustc_middle::thir::ExprKind::Call { fun, .. } => format!(" callee={}", thir[*fun].ty),
            _ => String::new(),
        };
        let _ = writeln!(lines, r#"{{"kind":"thir","expr":"{kind}","type":"{}"{}}}"#, expr.ty,
            if detail.is_empty() { String::new() } else { format!(r#","detail":"{}""#, detail.trim()) });
    }
}

struct Plain;
impl rustc_driver::Callbacks for Plain {}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    // As a workspace wrapper, Cargo passes the real rustc as the first argument.
    if args.get(1).is_some_and(|a| a.ends_with("rustc")) {
        args.remove(1);
    }
    let crate_name = args
        .iter()
        .position(|a| a == "--crate-name")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let target = std::env::var("LOCUS_SPIKE_CRATE").ok();
    if crate_name.is_some() && crate_name == target {
        let from = std::env::var("LOCUS_SPIKE_ROOT_FROM").expect("LOCUS_SPIKE_ROOT_FROM");
        let to = std::env::var("LOCUS_SPIKE_ROOT_TO").expect("LOCUS_SPIKE_ROOT_TO");
        for arg in args.iter_mut() {
            if *arg == from {
                *arg = to.clone();
            }
        }
        let out = std::env::var("LOCUS_SPIKE_FACTS").expect("LOCUS_SPIKE_FACTS");
        rustc_driver::run_compiler(&args, &mut Facts { out: out.into() });
    } else {
        rustc_driver::run_compiler(&args, &mut Plain);
    }
}
