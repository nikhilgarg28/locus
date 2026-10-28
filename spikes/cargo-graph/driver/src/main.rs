//! Spike: `locus-driver` for build mode. Cargo runs it as `RUSTC_WORKSPACE_WRAPPER`
//! or `RUSTC_WRAPPER`. A crate is checked when its root is a `.lc` file or when
//! one of its `--extern` crates has an interface file. For a checked crate, one
//! invocation:
//!
//! 1. lowers the crate's `.lc` files (`locus lower`, a separate process);
//! 2. compiles the checking program: the lowered text, with `--cfg __locus_check`
//!    and `LOCUS_FLAVOR=check`, metadata only, into a side directory, and writes
//!    facts after analysis;
//! 3. runs the logical pass (`locus check`, a separate process), which writes
//!    the crate's interface beside its artifacts;
//! 4. compiles the runtime program: Cargo's arguments, with the lowered root
//!    and Locus's library added, and stops it unless it defines the same items,
//!    by DefPathHash, as the checking program (a stand-in for validation);
//! 5. rewrites rustc's dep-info so that Cargo tracks the `.lc` sources, the
//!    dependencies' interfaces, Locus's library, and `locus` itself.
//!
//! Every other invocation is plain rustc. Environment:
//! - LOCUS_SPIKE_DIR (required for checked crates): where lowered text, side
//!   artifacts, facts, and the log go. LOCUS_SPIKE_LIB: Locus's library rlib.
//!   LOCUS_SPIKE_CHECK_LIB: the checking version of the library, when it is a
//!   different crate (options (i) and (ii) of question 1).
//! - Experiments only: LOCUS_SPIKE_REMAP (compile the checking program against
//!   the dependencies' checking metadata, option (i)), LOCUS_SPIKE_SKIP_CHECK
//!   (skip steps 2-3), LOCUS_SPIKE_RAW_DEPINFO (skip step 5),
//!   LOCUS_SPIKE_KEEP_ARTIFACT_JSON (let the checking compile tell Cargo about
//!   its metadata), LOCUS_SPIKE_SLOW_PASS (milliseconds to sleep in step 3),
//!   LOCUS_SPIKE_VV_STAMP (a line to add to `rustc -vV`), LOCUS_SPIKE_ARGS
//!   (write argument lists for this crate), LOCUS_SPIKE_SEEN (log what each
//!   invocation says about its crate).
#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_span;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};
use std::time::{Instant, SystemTime};

use rustc_driver::Compilation;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind};
use rustc_middle::ty::{TyCtxt, TypeckResults};
use rustc_span::def_id::{DefId, LOCAL_CRATE};

struct Plain;
impl rustc_driver::Callbacks for Plain {}

/// The runtime program: logs its crate identity and checks that it defines
/// the same items, by DefPathHash, as the checking program did. This stands in
/// for validating the runtime program against the checking program.
struct Runtime {
    checked: Option<Vec<String>>,
}

impl rustc_driver::Callbacks for Runtime {
    fn after_analysis<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let mine = items(tcx);
        let verdict = match &self.checked {
            None => "no checking program to compare with".to_string(),
            Some(theirs) if *theirs == mine => {
                format!("{} items, the same DefPathHashes as the checking program", mine.len())
            }
            Some(theirs) => {
                let missing = theirs.iter().filter(|i| !mine.contains(i)).count();
                let extra = mine.iter().filter(|i| !theirs.contains(i)).count();
                tcx.dcx().err(format!(
                    "locus: the runtime program differs from the checking program: {missing} items missing, {extra} extra"
                ));
                format!("{} items, DIFFERENT from the checking program ({missing} missing, {extra} extra)", mine.len())
            }
        };
        log(&format!(
            "runtime program {}: stable_crate_id {:016x}, svh {}, {verdict}",
            tcx.crate_name(LOCAL_CRATE),
            tcx.stable_crate_id(LOCAL_CRATE).as_u64(),
            tcx.crate_hash(LOCAL_CRATE)
        ));
        if self.checked.as_ref().is_some_and(|t| *t != mine) {
            return Compilation::Stop;
        }
        Compilation::Continue
    }
}

struct Facts {
    out: PathBuf,
    items: Vec<String>,
}

impl rustc_driver::Callbacks for Facts {
    fn after_analysis<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let mut lines = String::new();
        let _ = writeln!(
            lines,
            r#"{{"kind":"crate","name":"{}","stable_crate_id":"{:016x}","svh":"{}"}}"#,
            tcx.crate_name(LOCAL_CRATE),
            tcx.stable_crate_id(LOCAL_CRATE).as_u64(),
            tcx.crate_hash(LOCAL_CRATE)
        );
        self.items = items(tcx);
        for item in &self.items {
            let _ = writeln!(lines, "{item}");
        }
        for def_id in tcx.hir_body_owners() {
            let typeck = tcx.typeck(def_id);
            let body = tcx.hir_body_owned_by(def_id);
            Finder { tcx, typeck, lines: &mut lines }.visit_body(body);
        }
        fs::create_dir_all(self.out.parent().unwrap()).expect("create facts directory");
        fs::write(&self.out, lines).expect("write facts");
        Compilation::Continue
    }
}

/// The crate's functions, structs, and impls, as fact lines. An impl also
/// gets a semantic identity: the trait it implements and its self type.
fn items(tcx: TyCtxt<'_>) -> Vec<String> {
    let mut out = Vec::new();
    for def_id in tcx.hir_crate_items(()).definitions() {
        let id = def_id.to_def_id();
        let kind = tcx.def_kind(id);
        if !matches!(kind, DefKind::Fn | DefKind::AssocFn | DefKind::Impl { .. } | DefKind::Struct) {
            continue;
        }
        let mut line = format!(r#"{{"kind":"item","def_kind":"{kind:?}",{}"#, ident(tcx, id));
        if let DefKind::Impl { .. } = kind {
            let self_ty = tcx.type_of(id).instantiate_identity().skip_normalization();
            let _ = write!(line, r#","self_type":"{self_ty}""#);
            if let Some(adt) = self_ty.ty_adt_def() {
                let _ = write!(line, r#","self_type_hash":"{}""#, dph(tcx, adt.did()));
            }
            if let Some(trait_ref) = tcx.impl_opt_trait_ref(id) {
                let t = trait_ref.instantiate_identity().skip_normalization().def_id;
                let _ = write!(
                    line,
                    r#","trait":"{}","trait_hash":"{}""#,
                    tcx.def_path_str(t),
                    dph(tcx, t)
                );
            }
        }
        line.push('}');
        out.push(line);
    }
    out
}

/// An item by definition: its visible path, its definition path qualified by
/// crate name and StableCrateId, and its DefPathHash.
fn ident(tcx: TyCtxt<'_>, id: DefId) -> String {
    format!(
        r#""path":"{}","def":"{}","def_path_hash":"{}""#,
        tcx.def_path_str(id),
        tcx.def_path_debug_str(id),
        dph(tcx, id)
    )
}

/// A DefPathHash, as the StableCrateId of its crate and a hash local to it.
fn dph(tcx: TyCtxt<'_>, id: DefId) -> String {
    let h = tcx.def_path_hash(id);
    format!("{:016x}:{:016x}", h.stable_crate_id().as_u64(), h.local_hash().as_u64())
}

struct Finder<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    lines: &'a mut String,
}

impl<'a, 'tcx> Visitor<'tcx> for Finder<'a, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        let callee = match expr.kind {
            ExprKind::Call(f, args) => match f.kind {
                ExprKind::Path(ref q) => match self.typeck.qpath_res(q, f.hir_id) {
                    Res::Def(_, id) => Some((id, args)),
                    _ => None,
                },
                _ => None,
            },
            ExprKind::MethodCall(_, _, args, _) => {
                self.typeck.type_dependent_def_id(expr.hir_id).map(|id| (id, args))
            }
            _ => None,
        };
        if let Some((id, args)) = callee {
            let krate = self.tcx.crate_name(id.krate);
            let at = self.tcx.sess.source_map().span_to_diagnostic_string(expr.span.source_callsite());
            // `observe` is defined in `__locus`, or in `locus_core` for option (ii).
            if matches!(krate.as_str(), "__locus" | "locus_core") && self.tcx.item_name(id).as_str() == "observe" {
                let place = match args {
                    [Expr { kind: ExprKind::AddrOf(_, _, inner), .. }] => *inner,
                    [arg] => arg,
                    _ => expr,
                };
                let ty = self.typeck.expr_ty(place);
                let at = self.tcx.sess.source_map().span_to_diagnostic_string(place.span);
                let adt = ty.ty_adt_def().map_or(String::new(), |d| format!(",{}", ident(self.tcx, d.did())));
                let _ = writeln!(self.lines, r#"{{"kind":"observation","at":"{at}","type":"{ty}"{adt}}}"#);
            } else if !id.is_local() && !matches!(krate.as_str(), "std" | "core" | "alloc") {
                let _ = writeln!(self.lines, r#"{{"kind":"call","at":"{at}",{}}}"#, ident(self.tcx, id));
            }
        }
        intravisit::walk_expr(self, expr);
    }
}

/// What the driver needs from Cargo's invocation.
#[derive(Default)]
struct Inv {
    crate_name: String,
    root: Option<String>,
    out_dir: PathBuf,
    extra: String,
    crate_types: Vec<String>,
    test: bool,
    externs: Vec<(String, PathBuf)>,
}

impl Inv {
    fn parse(args: &[String]) -> Inv {
        const VALUED: &[&str] = &[
            "--crate-name", "--crate-type", "--out-dir", "-o", "--extern", "-L", "-C", "--cfg",
            "--check-cfg", "--cap-lints", "--target", "--print", "-l", "-W", "-A", "-D", "-F",
            "--force-warn", "--remap-path-prefix", "--sysroot", "-Z", "--explain",
        ];
        let mut inv = Inv::default();
        let mut i = 1;
        while i < args.len() {
            let a = args[i].as_str();
            if VALUED.contains(&a) {
                let v = args.get(i + 1).cloned().unwrap_or_default();
                match a {
                    "--crate-name" => inv.crate_name = v,
                    "--crate-type" => inv.crate_types.push(v),
                    "--out-dir" => inv.out_dir = v.into(),
                    "--extern" => {
                        if let Some((n, p)) = v.split_once('=') {
                            inv.externs.push((n.to_string(), p.into()));
                        }
                    }
                    "-C" => {
                        if let Some(x) = v.strip_prefix("extra-filename=") {
                            inv.extra = x.to_string();
                        }
                    }
                    _ => {}
                }
                i += 2;
                continue;
            }
            if a == "--test" {
                inv.test = true;
            } else if !a.starts_with('-') && inv.root.is_none() {
                inv.root = Some(a.to_string());
            }
            i += 1;
        }
        inv
    }

    fn kind(&self) -> &'static str {
        if self.crate_name == "build_script_build" {
            "build-script"
        } else if self.test {
            "test-harness"
        } else if self.crate_types.iter().any(|t| t == "proc-macro") {
            "proc-macro"
        } else if self.crate_types.iter().any(|t| t == "bin") {
            "bin"
        } else {
            "lib"
        }
    }

    fn is_lib(&self) -> bool {
        !self.test && self.crate_types.iter().any(|t| t == "lib" || t == "rlib")
    }

    /// What Cargo's invocation says about the crate, for question 5.
    fn seen(&self, args: &[String]) -> String {
        let value = |flag: &str| -> Vec<&str> {
            args.windows(2).filter(|w| w[0] == flag).map(|w| w[1].as_str()).collect()
        };
        let cfgs: Vec<&str> = value("--cfg").into_iter().filter(|c| !c.starts_with("feature=")).collect();
        let features: Vec<&str> = value("--cfg").into_iter().filter_map(|c| c.strip_prefix("feature=")).collect();
        let externs: Vec<&str> = self.externs.iter().map(|(n, _)| n.as_str()).collect();
        let emit = args.iter().find_map(|a| a.strip_prefix("--emit=")).unwrap_or("-");
        let root = self.root.as_deref().unwrap_or("-");
        let root = root.rsplit_once("/src/").map_or(root, |(_, r)| r);
        let rlib = self.out_dir.join(format!("lib{}.rlib", self.key()));
        format!(
            "root {root}, crate-type {}, --test {}, emit {emit}, cfg {cfgs:?}, features {features:?}, externs {externs:?}, cap-lints {}, CARGO_PRIMARY_PACKAGE {}{}",
            self.crate_types.join(","),
            if self.test { "yes" } else { "no" },
            value("--cap-lints").first().copied().unwrap_or("-"),
            if std::env::var("CARGO_PRIMARY_PACKAGE").is_ok() { "set" } else { "unset" },
            if !self.is_lib() { "" } else if rlib.exists() { ", an rlib already on disk" } else { ", no rlib on disk yet" },
        )
    }

    fn key(&self) -> String {
        format!("{}{}", self.crate_name, self.extra)
    }
}

/// A dependency's interface lives beside its artifact.
fn interface_of(artifact: &Path) -> PathBuf {
    artifact.with_extension("lcif")
}

fn compile(args: &[String], callbacks: &mut (dyn rustc_driver::Callbacks + Send)) -> i32 {
    // rustc reports errors by raising a fatal error once it has emitted them.
    match rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(args, callbacks)) {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

fn log(line: &str) {
    use std::io::Write;
    let ms = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() % 100_000;
    if let Ok(dir) = std::env::var("LOCUS_SPIKE_DIR")
        && let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(Path::new(&dir).join("log"))
    {
        // One write per line, so lines from parallel compiles do not interleave.
        let _ = f.write_all(format!("[{ms:05}] {line}\n").as_bytes());
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    // As a wrapper, Cargo passes the real rustc as the first argument.
    if args.get(1).is_some_and(|a| a.ends_with("rustc")) {
        args.remove(1);
    }
    if args[1..] == ["-vV"] {
        let code = compile(&args, &mut Plain);
        let stamp = std::env::var("LOCUS_SPIKE_VV_STAMP").ok();
        if let Some(s) = &stamp {
            println!("locus-driver: {s}");
        }
        log(&format!("rustc -vV{}", if stamp.is_some() { ", stamped" } else { "" }));
        exit(code);
    }
    let inv = Inv::parse(&args);
    if std::env::var("LOCUS_SPIKE_SEEN").is_ok() && !inv.crate_name.is_empty() && inv.crate_name != "___" {
        log(&format!("sees {} {}: {}", inv.kind(), inv.key(), inv.seen(&args)));
    }
    let deps: Vec<(String, PathBuf)> = inv
        .externs
        .iter()
        .map(|(n, p)| (n.clone(), interface_of(p)))
        .filter(|(_, i)| i.exists())
        .collect();
    let locus_root = inv.root.as_ref().is_some_and(|r| r.ends_with(".lc"));
    if !locus_root && deps.is_empty() {
        if !inv.crate_name.is_empty() && inv.crate_name != "___" {
            log(&format!("{} {}: plain rustc", inv.kind(), inv.key()));
        }
        exit(compile(&args, &mut Plain));
    }
    exit(build_checked(&args, &inv, &deps));
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        // Fail closed: a crate that must be checked is never compiled unchecked.
        eprintln!("locus-driver: error: {name} is not set, so this crate cannot be checked");
        exit(1)
    })
}

fn build_checked(args: &[String], inv: &Inv, deps: &[(String, PathBuf)]) -> i32 {
    let dir = PathBuf::from(required_env("LOCUS_SPIKE_DIR"));
    let lib = PathBuf::from(required_env("LOCUS_SPIKE_LIB"));
    let check_lib = std::env::var("LOCUS_SPIKE_CHECK_LIB").map_or_else(|_| lib.clone(), PathBuf::from);
    let locus = std::env::current_exe().unwrap().with_file_name("locus");
    let key = inv.key();
    let root = inv.root.clone().unwrap();
    let cwd = std::env::current_dir().unwrap();
    let start = Instant::now();
    log(&format!("{} {key}: start, {} dependency interfaces", inv.kind(), deps.len()));

    // 1. Lowering.
    let lowered_dir = dir.join("lower").join(&key);
    let src_dir = cwd.join(&root).parent().unwrap().to_path_buf();
    let lowered_root = if root.ends_with(".lc") {
        let lowered = Command::new(&locus).arg("lower").arg(&root).arg(&lowered_dir).status();
        if !lowered.is_ok_and(|s| s.success()) {
            eprintln!("locus-driver: error: lowering `{key}` failed");
            return 1;
        }
        let stem = Path::new(&root).file_stem().unwrap();
        Some(lowered_dir.join(stem).with_extension("rs").display().to_string())
    } else {
        None
    };
    let program_root = lowered_root.clone().unwrap_or_else(|| root.clone());
    let lib_args = |lib: &Path| {
        [
            "--extern".to_string(),
            format!("__locus={}", lib.display()),
            "-L".to_string(),
            format!("dependency={}", lib.parent().unwrap().display()),
        ]
    };

    // 2 and 3. The checking program and the logical pass.
    let (mut check_ms, mut pass_ms) = (0, 0);
    let mut check_args = Vec::new();
    let mut checked_items = None;
    if std::env::var("LOCUS_SPIKE_SKIP_CHECK").is_err() {
        check_args = checking_args(args, inv, &program_root, &dir, &lib_args(&check_lib));
        unsafe { std::env::set_var("LOCUS_FLAVOR", "check") };
        let facts = dir.join("facts").join(format!("{key}.jsonl"));
        let t = Instant::now();
        let mut callbacks = Facts { out: facts.clone(), items: Vec::new() };
        let code = compile(&check_args, &mut callbacks);
        checked_items = Some(callbacks.items);
        check_ms = t.elapsed().as_millis();
        if code != 0 {
            log(&format!("{} {key}: checking program FAILED to compile", inv.kind()));
            return code;
        }
        let t = Instant::now();
        if let Ok(ms) = std::env::var("LOCUS_SPIKE_SLOW_PASS") {
            std::thread::sleep(std::time::Duration::from_millis(ms.parse().unwrap()));
        }
        let mut pass = Command::new(&locus);
        pass.args(["check", "--crate", &inv.crate_name, "--facts"]).arg(&facts);
        if lowered_root.is_some() {
            pass.arg("--src").arg(&root);
        }
        if inv.is_lib() {
            pass.arg("--iface").arg(interface_of(&inv.out_dir.join(format!("lib{key}.rmeta"))));
        }
        for (name, iface) in deps {
            pass.arg("--dep").arg(format!("{name}={}", iface.display()));
        }
        let passed = pass.status().is_ok_and(|s| s.success());
        pass_ms = t.elapsed().as_millis();
        if !passed {
            log(&format!("{} {key}: logical pass FAILED; runtime program not compiled", inv.kind()));
            return 1;
        }
        log(&format!("{} {key}: checked", inv.kind()));
    }

    // 4. The runtime program: Cargo's arguments, the lowered root, Locus's library.
    let mut run_args: Vec<String> = args
        .iter()
        .map(|a| if Some(a) == inv.root.as_ref() { program_root.clone() } else { a.clone() })
        .collect();
    run_args.extend(lib_args(&lib));
    run_args.extend(["--check-cfg".to_string(), "cfg(__locus_check)".to_string()]);
    unsafe { std::env::set_var("LOCUS_FLAVOR", "run") };
    let t = Instant::now();
    let code = compile(&run_args, &mut Runtime { checked: checked_items });
    let run_ms = t.elapsed().as_millis();

    if std::env::var("LOCUS_SPIKE_ARGS").is_ok_and(|c| c == inv.crate_name) {
        let d = dir.join("args");
        fs::create_dir_all(&d).unwrap();
        for (name, list) in [("cargo", args), ("check", &check_args[..]), ("run", &run_args[..])] {
            fs::write(d.join(format!("{key}.{name}")), list[1..].join("\n") + "\n").unwrap();
        }
    }

    // 5. Dep-info.
    if code == 0 && std::env::var("LOCUS_SPIKE_RAW_DEPINFO").is_err() {
        let mut extra: Vec<PathBuf> = deps.iter().map(|(_, i)| i.clone()).collect();
        extra.push(lib.clone());
        if check_lib != lib {
            extra.push(check_lib.clone());
        }
        extra.push(locus.clone());
        let lowered = lowered_root.as_ref().map(|_| (lowered_dir.as_path(), src_dir.as_path()));
        rewrite_dep_info(inv, lowered, &extra);
    }
    log(&format!(
        "{} {key}: lower {}ms, checking program {check_ms}ms, logical pass {pass_ms}ms, runtime program {run_ms}ms{}",
        inv.kind(),
        start.elapsed().as_millis() - check_ms - pass_ms - run_ms,
        if code == 0 { "" } else { ", runtime program FAILED" },
    ));
    code
}

fn checking_args(args: &[String], inv: &Inv, root: &str, dir: &Path, lib_args: &[String]) -> Vec<String> {
    let keep_artifacts = std::env::var("LOCUS_SPIKE_KEEP_ARTIFACT_JSON").is_ok();
    let remap = std::env::var("LOCUS_SPIKE_REMAP").is_ok();
    let side = dir.join("check");
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if Some(a) == inv.root.as_ref() {
            out.push(root.to_string());
        } else if remap
            && a == "--extern"
            && let Some((name, path)) = args.get(i + 1).and_then(|v| v.split_once('='))
            && let Some(stem) = Path::new(path).file_stem()
            && side.join(stem).with_extension("rmeta").exists()
        {
            // Option (i): a checking program uses its dependencies' checking metadata.
            let checking = side.join(stem).with_extension("rmeta");
            out.extend([a.clone(), format!("{name}={}", checking.display())]);
            i += 1;
        } else if a.starts_with("--emit") {
            out.push("--emit=metadata".to_string());
        } else if a == "--out-dir" {
            out.extend([a.clone(), dir.join("check").display().to_string()]);
            i += 1;
        } else if a == "--cap-lints" {
            i += 1;
        } else if a == "-C" && args.get(i + 1).is_some_and(|v| v.starts_with("incremental=")) {
            let incr = dir.join("incremental").join(inv.key());
            out.extend([a.clone(), format!("incremental={}", incr.display())]);
            i += 1;
        } else if let Some(kinds) = a.strip_prefix("--json=")
            && !keep_artifacts
        {
            // Cargo starts dependents when it sees an artifact notification for
            // metadata; the checking program's metadata must not trigger that.
            let kept: Vec<&str> = kinds.split(',').filter(|k| !matches!(*k, "artifacts" | "future-incompat")).collect();
            if !kept.is_empty() {
                out.push(format!("--json={}", kept.join(",")));
            }
        } else {
            out.push(a.clone());
        }
        i += 1;
    }
    out.extend(["--cap-lints", "allow", "--cfg", "__locus_check"].map(String::from));
    out.extend(lib_args.iter().cloned());
    if remap {
        out.extend(["-L".to_string(), format!("dependency={}", side.display())]);
    }
    out
}

/// Rewrites the dep-info that rustc wrote for the runtime program: lowered
/// files become their `.lc` sources, and `extra` files are added. Cargo reads
/// only the first `target: deps` line.
fn rewrite_dep_info(inv: &Inv, lowered: Option<(&Path, &Path)>, extra: &[PathBuf]) {
    let path = inv.out_dir.join(format!("{}.d", inv.key()));
    let Ok(text) = fs::read_to_string(&path) else { return };
    let map = |p: &str| match lowered.and_then(|(low, src)| Path::new(p).strip_prefix(low).ok().map(|r| (r, src))) {
        Some((rel, src)) => src.join(rel).with_extension("lc").display().to_string(),
        None => p.to_string(),
    };
    let mut out = String::new();
    let mut first = true;
    for line in text.lines() {
        if let Some((target, deps)) = line.split_once(": ")
            && !line.starts_with('#')
        {
            let mut deps: Vec<String> = deps.split_whitespace().map(map).collect();
            if first {
                deps.extend(extra.iter().map(|p| p.display().to_string()));
                first = false;
            }
            let _ = writeln!(out, "{target}: {}", deps.join(" "));
        } else if let Some(dep) = line.strip_suffix(':') {
            let _ = writeln!(out, "{}:", map(dep));
        } else {
            let _ = writeln!(out, "{line}");
        }
    }
    for p in extra {
        let _ = writeln!(out, "{}:", p.display());
    }
    fs::write(&path, out).expect("rewrite dep-info");
}
