//! Spike: question 4. A rustc replacement that compiles a checking program
//! and prints, for the code written in it (not in macro expansions):
//!
//! - each call and method call, with the callee rustc resolved it to;
//! - each argument, with its parameter's declared type and the type it was
//!   instantiated with, marking parameters whose declared type is a marker
//!   (a logical parameter) and generic parameters that markers flow into;
//! - each struct literal field whose declared type is a marker;
//! - each local whose type holds a marker;
//! - each function item that has a marker parameter and is used as a value.
//!
//! Markers are recognized by identity: an ADT named `Erased` or `ErasedData`
//! defined in the crate `__locus`. Usage: `locus-lowering-facts <rustc args>`.
//! Facts go to stdout; rustc's own diagnostics go to stderr as usual.
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_driver::Compilation;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, Pat, PatKind};
use rustc_middle::ty::{self, GenericArgKind, Ty, TyCtxt, TypeckResults};
use rustc_span::Span;
use rustc_span::def_id::DefId;

struct Facts;

impl rustc_driver::Callbacks for Facts {
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        for def_id in tcx.hir_body_owners() {
            let span = tcx.def_span(def_id);
            if span.from_expansion() {
                continue;
            }
            let typeck = tcx.typeck(def_id);
            let body = tcx.hir_body_owned_by(def_id);
            println!("body {}", tcx.def_path_str(def_id));
            let mut finder = Finder {
                tcx,
                typeck,
                callees: HashSet::new(),
            };
            finder.visit_body(body);
        }
        // Continue: rustc's own analysis, including borrow checking, still runs.
        Compilation::Continue
    }
}

struct Finder<'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    callees: HashSet<HirId>,
}

/// Whether `ty` is a marker, or holds one at any depth.
fn marker(tcx: TyCtxt<'_>, ty: Ty<'_>) -> Option<&'static str> {
    for arg in ty.walk() {
        if let GenericArgKind::Type(t) = arg.kind()
            && let ty::Adt(adt, _) = t.kind()
            && tcx.crate_name(adt.did().krate).as_str() == "__locus"
        {
            match tcx.item_name(adt.did()).as_str() {
                "Erased" => return Some("Erased"),
                "ErasedData" => return Some("ErasedData"),
                _ => {}
            }
        }
    }
    None
}

fn is_marker(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    matches!(ty.kind(), ty::Adt(..)) && marker(tcx, ty).is_some() && ty.walk().count() == 1
}

impl<'tcx> Finder<'tcx> {
    fn at(&self, span: Span) -> String {
        let loc = self.tcx.sess.source_map().lookup_char_pos(span.lo());
        format!("{}:{}", loc.line, loc.col.0 + 1)
    }

    /// The source text, at the call site for code a macro wrote.
    fn text(&self, span: Span) -> String {
        self.tcx
            .sess
            .source_map()
            .span_to_snippet(span.source_callsite())
            .unwrap_or_else(|_| "?".into())
    }

    /// One line per argument: what fills which parameter.
    fn arguments(&self, callee: DefId, generics: ty::GenericArgsRef<'tcx>, args: &[(String, Span)]) {
        let sig = self.tcx.fn_sig(callee);
        let declared = sig.instantiate_identity().skip_binder();
        let actual = sig.instantiate(self.tcx, generics).skip_binder();
        for (i, (label, span)) in args.iter().enumerate() {
            let (Some(d), Some(a)) = (declared.inputs().get(i), actual.inputs().get(i)) else {
                println!("    {label} `{}`: no parameter", self.text(*span));
                continue;
            };
            let verdict = if is_marker(self.tcx, *d) {
                "logical parameter".to_string()
            } else if marker(self.tcx, *d).is_some() {
                "parameter type holds markers".to_string()
            } else if marker(self.tcx, *a).is_some() {
                format!("markers flow into generic parameter (instantiated {a})")
            } else {
                "runtime".to_string()
            };
            println!("    {label} `{}`: declared {d}: {verdict}", self.text(*span));
        }
        let (d, a) = (declared.output(), actual.output());
        if marker(self.tcx, a).is_some() {
            println!("    result: declared {d}, instantiated {a}");
        }
    }
}

impl<'tcx> Visitor<'tcx> for Finder<'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if !expr.span.from_expansion() {
            match expr.kind {
                ExprKind::Call(callee, args) => {
                    self.callees.insert(callee.hir_id);
                    let fty = self.typeck.node_type(callee.hir_id);
                    let labelled: Vec<_> = args
                        .iter()
                        .enumerate()
                        .map(|(i, a)| (format!("arg {i}"), a.span))
                        .collect();
                    match *fty.kind() {
                        ty::FnDef(def_id, generics) => {
                            println!(
                                "  {} call `{}` resolved to `{}`",
                                self.at(expr.span),
                                self.text(expr.span),
                                self.tcx.def_path_str_with_args(def_id, generics)
                            );
                            self.arguments(def_id, generics, &labelled);
                        }
                        _ => println!(
                            "  {} call `{}` through a value of type `{fty}`",
                            self.at(expr.span),
                            self.text(expr.span)
                        ),
                    }
                }
                ExprKind::MethodCall(seg, receiver, args, _) => match self.typeck.type_dependent_def(expr.hir_id) {
                    Some((_, def_id)) => {
                        let generics = self.typeck.node_args(expr.hir_id);
                        println!(
                            "  {} method `{}` resolved to `{}`",
                            self.at(expr.span),
                            seg.ident,
                            self.tcx.def_path_str_with_args(def_id, generics)
                        );
                        let mut labelled = vec![("self".to_string(), receiver.span)];
                        labelled.extend(args.iter().enumerate().map(|(i, a)| (format!("arg {}", i + 1), a.span)));
                        self.arguments(def_id, generics, &labelled);
                    }
                    None => println!("  {} method `{}`: not resolved", self.at(expr.span), seg.ident),
                },
                ExprKind::Struct(_, fields, _) => {
                    if let ty::Adt(adt, generics) = self.typeck.expr_ty(expr).kind() {
                        for f in fields {
                            if let Some(def) = adt.all_fields().find(|d| d.name == f.ident.name) {
                                let declared = self.tcx.type_of(def.did).instantiate_identity().skip_normalization();
                                if marker(self.tcx, declared).is_some() {
                                    println!(
                                        "  {} field `{}` of `{}`: declared {declared}, filled by `{}`",
                                        self.at(f.span),
                                        f.ident,
                                        self.tcx.def_path_str_with_args(adt.did(), generics),
                                        self.text(f.expr.span)
                                    );
                                }
                            }
                        }
                    }
                }
                ExprKind::Path(_) if !self.callees.contains(&expr.hir_id) => {
                    let ty = self.typeck.node_type(expr.hir_id);
                    if let ty::FnDef(def_id, _) = *ty.kind()
                        && self
                            .tcx
                            .fn_sig(def_id)
                            .instantiate_identity()
                            .skip_binder()
                            .inputs()
                            .iter()
                            .any(|t| marker(self.tcx, *t).is_some())
                    {
                        let adjusted = self.typeck.expr_ty_adjusted(expr);
                        println!(
                            "  {} function `{}` with a marker parameter used as a value, as `{adjusted}`",
                            self.at(expr.span),
                            self.tcx.def_path_str(def_id)
                        );
                    }
                }
                _ => {}
            }
        }
        intravisit::walk_expr(self, expr);
    }

    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        if !pat.span.from_expansion()
            && let PatKind::Binding(_, _, ident, _) = pat.kind
        {
            let ty = self.typeck.node_type(pat.hir_id);
            if marker(self.tcx, ty).is_some() {
                println!("  {} local `{ident}`: {ty}", self.at(pat.span));
            }
        }
        intravisit::walk_pat(self, pat);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    rustc_driver::run_compiler(&args, &mut Facts);
}
