//! Spike: rustc's trait solver as an oracle. After rustc has checked a crate, this driver builds
//! types that appear nowhere in the crate's code and asks rustc which implementation a member
//! comes from, whether a bound holds, and what an associated type is.
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_trait_selection;

use rustc_driver::Compilation;
use rustc_hir::def_id::DefId;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::ty::{self, GenericArg, Ty, TyCtxt};
use rustc_trait_selection::infer::InferCtxtExt;

struct Oracle;

impl rustc_driver::Callbacks for Oracle {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        ask(tcx);
        Compilation::Stop
    }
}

fn item(tcx: TyCtxt<'_>, name: &str) -> DefId {
    for id in tcx.hir_crate_items(()).definitions() {
        if tcx.def_path_str(id) == name {
            return id.to_def_id();
        }
    }
    panic!("no item named {name}");
}

fn member(tcx: TyCtxt<'_>, owner: DefId, name: &str) -> DefId {
    tcx.associated_items(owner)
        .in_definition_order()
        .find(|i| i.name().as_str() == name)
        .unwrap_or_else(|| panic!("no member {name}"))
        .def_id
}

fn ask<'tcx>(tcx: TyCtxt<'tcx>) {
    let adt = |name: &str, args: &[Ty<'tcx>]| -> Ty<'tcx> {
        let args = tcx.mk_args_from_iter(args.iter().map(|t| GenericArg::from(*t)));
        tcx.type_of(item(tcx, name)).instantiate(tcx, args).skip_normalization()
    };
    let int = adt("Int", &[]);
    let nat = adt("Nat", &[]);
    let boolean = adt("Bool", &[]);
    let version = adt("Version", &[]);
    let seq_version = adt("Seq", &[version]);
    let seq_seq_nat = adt("Seq", &[adt("Seq", &[nat])]);
    let seq_bool = adt("Seq", &[boolean]);

    let env = ty::TypingEnv::fully_monomorphized();
    let ord = item(tcx, "OrdLogic");
    let le = member(tcx, ord, "le");
    let family = item(tcx, "Family");
    let out = member(tcx, family, "Out");

    println!("which implementation a member comes from:");
    for self_ty in [int, version, seq_version, seq_seq_nat] {
        let args = tcx.mk_args(&[GenericArg::from(self_ty)]);
        match ty::Instance::try_resolve(tcx, env, le, args) {
            Ok(Some(instance)) => {
                let span = tcx.def_span(instance.def_id());
                let line = tcx.sess.source_map().lookup_char_pos(span.lo()).line;
                println!(
                    "  <{self_ty} as OrdLogic>::le  is the `le` on line {line} of the sample, at {:?}",
                    instance.args
                )
            }
            _ => println!("  <{self_ty} as OrdLogic>::le  has no implementation"),
        }
    }

    println!("whether a bound holds:");
    let infcx = tcx.infer_ctxt().build(ty::TypingMode::non_body_analysis());
    for self_ty in [version, seq_version, seq_seq_nat, boolean, seq_bool] {
        let holds = infcx
            .type_implements_trait(ord, [self_ty], ty::ParamEnv::empty())
            .must_apply_modulo_regions();
        println!("  {self_ty}: OrdLogic  is  {holds}");
    }

    println!("what an associated type is:");
    for self_ty in [int, nat, seq_seq_nat] {
        let projection = Ty::new_projection(tcx, out, [self_ty]);
        let normal = tcx.normalize_erasing_regions(env, ty::Unnormalized::new(projection));
        println!("  <{self_ty} as Family>::Out  is  {normal}");
    }

    println!("under a bound, in `fn generic<T: OrdLogic>()`:");
    let generic = item(tcx, "generic");
    let t = ty::GenericArgs::identity_for_item(tcx, generic).type_at(0);
    let seq_t = adt("Seq", &[t]);
    let holds = infcx
        .type_implements_trait(ord, [seq_t], tcx.param_env(generic))
        .must_apply_modulo_regions();
    println!("  {seq_t}: OrdLogic  is  {holds}");
    let holds = infcx
        .type_implements_trait(family, [t], tcx.param_env(generic))
        .must_apply_modulo_regions();
    println!("  {t}: Family  is  {holds}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    rustc_driver::run_compiler(&args, &mut Oracle);
}
