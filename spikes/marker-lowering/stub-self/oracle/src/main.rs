//! The spike's driver for its second mode. Adapted (copied, not depended on)
//! from `spikes/logical-markers/oracle`.
//!
//! Cargo runs it in place of rustc (`RUSTC_WORKSPACE_WRAPPER`). For every
//! crate but one it is rustc. For the crate named by `LOCUS_ASK_CRATE`, once
//! rustc has checked it, it reads the questions in the file `LOCUS_ASK_FILE`
//! (lines `//! ask: QUESTION => EXPECTED`), builds the types each names, and
//! asks rustc's trait solver. A logical type's name stands for its stub,
//! `__locus_Name`, which the program's own code names only in `impl`s.
//!
//!   bound TYPE: TRAIT              whether the bound holds
//!   impl TYPE: TRAIT               which implementation rustc selects
//!   assoc TYPE: TRAIT::Name        what the associated type is
//!   in FUNCTION: one of the above  the same under that function's bounds,
//!                                  where its type parameters are types
//!
//! `Snap<TYPE>` is the snapshot family of the library, `<TYPE as SnapOf>::S`,
//! normalized: a question about `Snap<Vec<u32>>` is one about `Seq<Snap<u32>>`.
//!
//! With `LOCUS_CALLS_OUT=file`, it writes instead a listing of the crate's
//! calls: for each call, method call, and use of an associated constant that
//! the source wrote, the item rustc resolved it to, the implementation that
//! item comes from where its type arguments say, and the type arguments.
//! Two expansions of one source correspond only if their listings agree.
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_trait_selection;

use rustc_driver::Compilation;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LOCAL_CRATE};
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::ty::{self, GenericArg, Ty, TyCtxt};
use rustc_trait_selection::infer::InferCtxtExt;
use std::fmt::Write as _;

struct Passive;
impl rustc_driver::Callbacks for Passive {}

/// The listing of calls.
struct Calls {
    out: String,
}

impl rustc_driver::Callbacks for Calls {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let mut lines = Vec::new();
        for owner in tcx.hir_body_owners() {
            let mut lister = Lister {
                tcx,
                owner: owner.to_def_id(),
                typeck: tcx.typeck(owner),
                lines: &mut lines,
            };
            rustc_hir::intravisit::Visitor::visit_body(&mut lister, tcx.hir_body_owned_by(owner));
        }
        lines.sort();
        lines.dedup();
        std::fs::write(&self.out, lines.join("\n") + "\n").expect("write the listing");
        Compilation::Continue
    }
}

struct Lister<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    owner: DefId,
    typeck: &'tcx ty::TypeckResults<'tcx>,
    lines: &'a mut Vec<String>,
}

impl<'tcx> Lister<'_, 'tcx> {
    /// Whether the source text at `span` is the name of the item.
    fn written(&self, span: rustc_span::Span, id: DefId) -> bool {
        let name = self.tcx.item_name(id);
        self.tcx
            .sess
            .source_map()
            .span_to_snippet(span)
            .is_ok_and(|text| text == name.as_str())
    }

    /// One line: where, the item, the implementation, the type arguments.
    fn record(&mut self, span: rustc_span::Span, id: DefId, args: ty::GenericArgsRef<'tcx>) {
        let tcx = self.tcx;
        // What the source wrote, and not what Locus's library is, nor what
        // lowering wrote: a probe, or an item it generated, carries the span
        // of a source token that is not its name.
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
        // written, for one of this crate, and its path otherwise. A path
        // can name a logical type, which the expansions write differently.
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
                    rustc_hir::def::Res::Def(
                        DefKind::Fn | DefKind::AssocFn | DefKind::AssocConst { .. },
                        id,
                    ),
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
}

struct Oracle {
    questions: Vec<String>,
    out: String,
}

impl rustc_driver::Callbacks for Oracle {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let mut text = String::new();
        for q in &self.questions {
            let answer = Asker { tcx, env: None }
                .ask(q)
                .unwrap_or_else(|e| format!("cannot be asked: {e}"));
            writeln!(text, "ask: {q} => {answer}").unwrap();
        }
        std::fs::write(&self.out, text).expect("write answers");
        Compilation::Continue
    }
}

/// An item of the crate being checked or of the library, by its name.
fn item(tcx: TyCtxt<'_>, name: &str, kinds: &[DefKind]) -> Option<DefId> {
    for id in tcx.hir_crate_items(()).definitions() {
        let id = id.to_def_id();
        if kinds.contains(&tcx.def_kind(id)) && tcx.item_name(id).as_str() == name {
            return Some(id);
        }
    }
    let library = tcx
        .crates(())
        .iter()
        .find(|c| tcx.crate_name(**c).as_str() == "locus")?;
    let mut modules = vec![library.as_def_id()];
    while let Some(module) = modules.pop() {
        for child in tcx.module_children(module) {
            let Some(id) = child.res.opt_def_id() else {
                continue;
            };
            if id.krate != *library {
                continue;
            }
            let kind = tcx.def_kind(id);
            if kinds.contains(&kind) && child.ident.name.as_str() == name {
                return Some(id);
            }
            if kind == DefKind::Mod && !modules.contains(&id) && id != module {
                modules.push(id);
            }
        }
    }
    None
}

const TYPES: &[DefKind] = &[DefKind::Struct, DefKind::Enum];

struct Asker<'tcx> {
    tcx: TyCtxt<'tcx>,
    /// The function whose bounds and type parameters are in scope.
    env: Option<DefId>,
}

/// `Name<A, B>` split into its name and its arguments.
fn split(text: &str) -> Result<(&str, Vec<&str>), String> {
    let text = text.trim();
    let Some(open) = text.find('<') else {
        return Ok((text, Vec::new()));
    };
    if !text.ends_with('>') {
        return Err(format!("`{text}` is not `Name<..>`"));
    }
    let (mut depth, mut start, mut args) = (0, open + 1, Vec::new());
    for (i, c) in text.char_indices().skip(open + 1) {
        match c {
            '<' | '(' => depth += 1,
            '>' if depth == 0 => args.push(text[start..i].trim()),
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(text[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    Ok((text[..open].trim(), args))
}

impl<'tcx> Asker<'tcx> {
    fn param_env(&self) -> ty::ParamEnv<'tcx> {
        match self.env {
            Some(f) => self.tcx.param_env(f),
            None => ty::ParamEnv::empty(),
        }
    }

    fn typing_env(&self) -> ty::TypingEnv<'tcx> {
        match self.env {
            Some(f) => ty::TypingEnv::post_analysis(self.tcx, f),
            None => ty::TypingEnv::fully_monomorphized(),
        }
    }

    fn adt(&self, id: DefId, args: &[Ty<'tcx>]) -> Ty<'tcx> {
        let args = self
            .tcx
            .mk_args_from_iter(args.iter().map(|t| GenericArg::from(*t)));
        self.tcx
            .type_of(id)
            .instantiate(self.tcx, args)
            .skip_normalization()
    }

    /// The type a question names. A logical type's name is its stub.
    fn ty(&self, text: &str) -> Result<Ty<'tcx>, String> {
        let tcx = self.tcx;
        let text = text.trim();
        if let Some(inner) = text.strip_prefix('(').and_then(|t| t.strip_suffix(')')) {
            let wrapped = format!("T<{inner}>");
            let (_, parts) = split(&wrapped)?;
            let parts = parts
                .into_iter()
                .map(|p| self.ty(p))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Ty::new_tup(tcx, &parts));
        }
        if let Some(inner) = text.strip_prefix("&mut ") {
            return Ok(Ty::new_mut_ref(
                tcx,
                tcx.lifetimes.re_erased,
                self.ty(inner)?,
            ));
        }
        if let Some(inner) = text.strip_prefix('&') {
            return Ok(Ty::new_imm_ref(
                tcx,
                tcx.lifetimes.re_erased,
                self.ty(inner)?,
            ));
        }
        let (name, args) = split(text)?;
        let args = args
            .into_iter()
            .map(|a| self.ty(a))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(f) = self.env {
            let generics = tcx.generics_of(f);
            for i in 0..generics.count() {
                let p = generics.param_at(i, tcx);
                if p.name.as_str() == name {
                    return Ok(ty::GenericArgs::identity_for_item(tcx, f).type_at(i));
                }
            }
        }
        match name {
            "u8" => return Ok(tcx.types.u8),
            "u32" => return Ok(tcx.types.u32),
            "usize" => return Ok(tcx.types.usize),
            "bool" => return Ok(tcx.types.bool),
            "Vec" => {
                let vec = tcx
                    .get_diagnostic_item(rustc_span::sym::Vec)
                    .ok_or("no `Vec`")?;
                let alloc = tcx
                    .type_of(tcx.generics_of(vec).param_at(1, tcx).def_id)
                    .instantiate(tcx, tcx.mk_args(&[GenericArg::from(args[0])]))
                    .skip_normalization();
                return Ok(self.adt(vec, &[args[0], alloc]));
            }
            "Snap" => {
                // The snapshot family, normalized.
                let family = item(tcx, "SnapOf", &[DefKind::Trait]).ok_or("no `SnapOf`")?;
                let s = self.member(family, "S")?;
                let projection = Ty::new_projection(tcx, s, [args[0]]);
                return Ok(tcx.normalize_erasing_regions(
                    self.typing_env(),
                    ty::Unnormalized::new(projection),
                ));
            }
            _ => {}
        }
        let id = item(tcx, &format!("__locus_{name}"), TYPES)
            .or_else(|| item(tcx, name, TYPES))
            .ok_or_else(|| format!("no type `{name}`, and no stub `__locus_{name}`"))?;
        Ok(self.adt(id, &args))
    }

    fn member(&self, owner: DefId, name: &str) -> Result<DefId, String> {
        self.tcx
            .associated_items(owner)
            .in_definition_order()
            .find(|i| i.name().as_str() == name)
            .map(|i| i.def_id)
            .ok_or_else(|| format!("no member `{name}`"))
    }

    /// `TYPE: TRAIT<ARGS>`: the trait, and its arguments with `Self` first.
    fn bound(&self, text: &str) -> Result<(DefId, Vec<Ty<'tcx>>), String> {
        let (ty, tr) = text
            .split_once(": ")
            .ok_or_else(|| format!("`{text}` is not `TYPE: TRAIT`"))?;
        let (name, args) = split(tr)?;
        let id =
            item(self.tcx, name, &[DefKind::Trait]).ok_or_else(|| format!("no trait `{name}`"))?;
        let mut all = vec![self.ty(ty)?];
        for a in args {
            all.push(self.ty(a)?);
        }
        Ok((id, all))
    }

    /// A type, with the names of stubs written as the logical types' names.
    fn show(&self, ty: impl std::fmt::Display) -> String {
        ty.to_string()
            .replace("locus::", "")
            .replace("__locus_", "")
    }

    fn ask(mut self, q: &str) -> Result<String, String> {
        let tcx = self.tcx;
        let mut q = q.trim();
        if let Some(rest) = q.strip_prefix("in ") {
            let (f, rest) = rest
                .split_once(": ")
                .ok_or("`in FUNCTION: QUESTION` expected")?;
            self.env = Some(
                item(tcx, f.trim(), &[DefKind::Fn]).ok_or_else(|| format!("no function `{f}`"))?,
            );
            q = rest.trim();
        }
        if let Some(rest) = q.strip_prefix("bound ") {
            let (tr, args) = self.bound(rest)?;
            let infcx = tcx.infer_ctxt().build(ty::TypingMode::non_body_analysis());
            let holds = infcx
                .type_implements_trait(tr, args, self.param_env())
                .must_apply_modulo_regions();
            return Ok(if holds { "holds" } else { "does not hold" }.to_string());
        }
        if let Some(rest) = q.strip_prefix("impl ") {
            let (tr, args) = self.bound(rest)?;
            let trait_ref = ty::TraitRef::new(tcx, tr, args);
            let input = self.typing_env().as_query_input(trait_ref);
            return Ok(match tcx.codegen_select_candidate(input) {
                Ok(rustc_middle::traits::ImplSource::UserDefined(data)) => {
                    let span = tcx.def_span(data.impl_def_id);
                    let at = if data.impl_def_id.krate == LOCAL_CRATE {
                        let line = tcx.sess.source_map().lookup_char_pos(span.lo()).line;
                        format!("the impl on line {line}")
                    } else {
                        "an impl of the library".to_string()
                    };
                    if data.args.is_empty() {
                        at
                    } else {
                        format!("{at}, at {}", self.show(format!("{:?}", data.args)))
                    }
                }
                Ok(rustc_middle::traits::ImplSource::Param(_)) => "the bound in scope".to_string(),
                Ok(rustc_middle::traits::ImplSource::Builtin(..)) => "built in".to_string(),
                Err(_) => "no implementation".to_string(),
            });
        }
        if let Some(rest) = q.strip_prefix("assoc ") {
            let (bound, name) = rest
                .rsplit_once("::")
                .ok_or("`assoc TYPE: TRAIT::Name` expected")?;
            let (tr, args) = self.bound(bound)?;
            let member = self.member(tr, name.trim())?;
            let projection = Ty::new_projection(tcx, member, args);
            let normal = tcx
                .try_normalize_erasing_regions(self.typing_env(), ty::Unnormalized::new(projection))
                .map_err(|_| "it does not normalize".to_string())?;
            return Ok(self.show(normal));
        }
        Err("not a question".to_string())
    }
}

fn main() {
    // `oracle /path/to/rustc ARGS..`, as Cargo runs a wrapper.
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
    let wanted = std::env::var("LOCUS_ASK_CRATE").ok();
    if let (true, Ok(out)) = (
        name.is_some() && name == wanted,
        std::env::var("LOCUS_CALLS_OUT"),
    ) {
        rustc_driver::run_compiler(&args, &mut Calls { out });
    } else if name.is_some() && name == wanted {
        let file = std::env::var("LOCUS_ASK_FILE").expect("LOCUS_ASK_FILE");
        let source = std::fs::read_to_string(&file).expect("read the case");
        let questions = source
            .lines()
            .filter_map(|l| l.strip_prefix("//! ask: "))
            .map(|l| l.split(" => ").next().unwrap().trim().to_string())
            .collect();
        let out = std::env::var("LOCUS_ASK_OUT").expect("LOCUS_ASK_OUT");
        rustc_driver::run_compiler(&args, &mut Oracle { questions, out });
    } else {
        rustc_driver::run_compiler(&args, &mut Passive);
    }
}
