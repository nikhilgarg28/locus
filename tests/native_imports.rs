//! Real native packages exercise extraction, binding, use and Rust execution.
use locus::{
    imports::{
        self,
        model::{Interface, Origin},
    },
    project::{Build, cargo},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.path().is_dir() {
            copy(&e.path(), &dest)
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}
fn fixture(name: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("native_imports_{name}"));
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native-imports"),
        &root,
    );
    root
}
fn build(root: &Path, code: &str) -> Build {
    let host = root.join("host");
    fs::write(host.join("export.lc"), code).unwrap();
    let mut b = Build::new(host.join("export.lc"))
        .offline(true)
        .out_dir(root.join("generated"));
    b.use_proofs = false;
    b
}
fn inspect(root: &Path, features: &[&str]) -> String {
    let mut args = vec![
        "renamed".into(),
        "--manifest-path".into(),
        root.join("host/Cargo.toml").into_os_string(),
        "--offline".into(),
        "--json".into(),
    ];
    if !features.is_empty() {
        args.extend(["--features".into(), features.join(",").into()]);
    }
    imports::command::run(&args).unwrap()
}
#[test]
#[doc = "spec: 1.30:1"]
#[doc = "spec: 1.30:2"]
fn all_entity_kinds_survive_inspection_with_foreign_provenance() {
    let root = fixture("inventory");
    let json: serde_json::Value = serde_json::from_str(&inspect(&root, &[])).unwrap();
    let entities = json["entities"].as_object().unwrap();
    for (name, kind) in [
        ("Token", "struct"),
        ("Event", "enum"),
        ("Raw", "union"),
        ("Alias", "type_alias"),
        ("Surface", "trait"),
        ("LIMIT", "constant"),
        ("identity", "macro"),
        ("future", "function"),
        ("dangerous", "function"),
        ("Item", "assoc_type"),
        ("amount", "function"),
        ("hidden_docs", "function"),
    ] {
        let e = entities
            .values()
            .find(|e| e["name"] == name && e["kind"] == kind)
            .unwrap_or_else(|| panic!("missing {kind} {name}"));
        assert_eq!(e["origin"]["language"], "rust");
    }
    assert_eq!(
        entities.values().filter(|e| e["name"] == "shared").count(),
        3
    );
    assert_eq!(json["interface_version"], 1);
    assert_eq!(json["rustdoc_version"], 56);
    assert!(
        !json.to_string().contains("RUSTC_BOOTSTRAP"),
        "published metadata must not contain captured environment values"
    );
    let checked = build(&root, "import renamed; fn f()->u8{1}")
        .check()
        .unwrap();
    assert!(
        checked
            .loaded
            .graph
            .items
            .iter()
            .enumerate()
            .any(|(i, _)| matches!(checked.loaded.graph.origin(i), Origin::Rust { .. }))
    );
}
#[test]
#[doc = "spec: 1.30:3"]
#[doc = "spec: 1.30:4"]
fn supported_calls_compile_preserve_effects_and_supply_no_proof() {
    let root = fixture("execute");
    let code = r#"
import renamed as native;
import crate::own;
pub fn answer()->u8 { let (out, yes)=native::api::pair(6); if yes { native::echo(out) } else { 0 } }
pub fn host_value()->u16 {own(8)}
pub fn pointer_values(n:usize,s:isize)->(usize,isize){native::pointer_values(n,s)}
pub fn namespaces()->u8{native::shared(4)}
pub fn argument_once()->u8{let (out,b)=native::pair(native::tick());out}
pub fn observable()->@(1==1) {native::tick();_}
pub fn fails()->u8 {native::fail()}
"#;
    let checked = build(&root, code).check().unwrap();
    let function = checked
        .checked
        .functions
        .iter()
        .find(|(name, _)| name.ends_with("_answer"))
        .unwrap()
        .1;
    let a = locus::exec::CheckInterpreter::new(checked.checked.session.program(), 1000)
        .call(function, vec![])
        .unwrap_err();
    let b = locus::erased::Interpreter::new(checked.checked.session.erased(), 1000)
        .call(function, vec![])
        .unwrap_err();
    assert_eq!(a, b);
    assert!(matches!(a, locus::erased::RunError::Native(_)));
    assert!(locus::audit::render(&checked.checked).contains("physical signature only"));
    let rust = locus::project::rust(checked).unwrap();
    fs::write(root.join("host/src/generated.rs"), rust).unwrap();
    fs::write(root.join("host/src/main.rs"),r#"
fn own(value:u16)->u16 {value.wrapping_add(1)}
mod generated {include!("generated.rs");}
fn main(){assert_eq!(generated::pointer_values(usize::MAX,isize::MIN),(usize::MAX,isize::MIN));assert_eq!(generated::answer(),7);assert_eq!(generated::namespaces(),4);assert_eq!(generated::host_value(),9);let before=renamed::count();generated::observable();assert_eq!(renamed::count(),before+1);let next=renamed::count();assert_eq!(generated::argument_once(),next+1);assert_eq!(renamed::count(),next+1);assert!(std::panic::catch_unwind(generated::fails).is_err());}
"#).unwrap();
    let result = Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("host/Cargo.toml"))
        .env("RUSTFLAGS", "-Dwarnings")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    // The runtime result is unconstrained. Plain import cannot establish this.
    let error = build(
        &root,
        "import renamed::echo; fn wrong()->@(1==0){let n=echo(1);_}",
    )
    .check()
    .err()
    .unwrap()
    .to_string();
    assert!(error.contains("L0230"), "{error}");
}
#[test]
#[doc = "spec: 1.30:5"]
fn unavailable_is_different_from_missing() {
    let root = fixture("unavailable");
    for (expr, phrase) in [
        ("n::future()", "async"),
        ("n::dangerous()", "unsafe"),
        ("n::generic(1)", "generic"),
        ("n::borrow(1)", "Rust type"),
        ("n::LIMIT", "constants"),
    ] {
        let error = build(&root, &format!("import renamed as n;fn f()->u8{{{expr}}}"))
            .check()
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("L0514") && error.contains(phrase), "{error}");
    }
    for code in [
        "import renamed::Typo;",
        "import renamed as n;fn f(x:n::Typo)->u8{0}",
    ] {
        let error = build(&root, code).check().err().unwrap().to_string();
        assert!(error.contains("L0513") && error.contains("Typo"), "{error}");
    }
    let error = build(&root, "import renamed::Token;fn f(x:Token)->u8{0}")
        .check()
        .err()
        .unwrap()
        .to_string();
    assert!(
        error.contains("L0514") && error.contains("opaque Rust types"),
        "{error}"
    );
    let error = build(&root, "pub import renamed;")
        .check()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("pub use"), "{error}");
}
#[test]
#[doc = "spec: 1.30:6"]
fn cfg_doc_does_not_authorize_native_calls() {
    let root = fixture("cfg_doc");
    // Metadata contains both items, and importing that metadata is allowed.
    build(&root, "import renamed;").check().unwrap();
    for method in ["doc_only()", "changed(3)"] {
        let error = build(
            &root,
            &format!("import renamed;fn f()->u8{{renamed::{method}}}"),
        )
        .check()
        .err()
        .unwrap()
        .to_string();
        assert!(
            error.contains("L0512")
                && error.contains("native Rust signature validation")
                && error.contains("cfg(doc)"),
            "{error}"
        );
    }
}
#[test]
#[doc = "spec: 1.30:7"]
fn cargo_features_source_and_receipts_are_not_reused_blindly() {
    let root = fixture("features");
    let plain: serde_json::Value = serde_json::from_str(&inspect(&root, &[])).unwrap();
    let extra: serde_json::Value = serde_json::from_str(&inspect(&root, &["extra"])).unwrap();
    let has = |v: &serde_json::Value| {
        v["entities"]
            .as_object()
            .unwrap()
            .values()
            .any(|e| e["name"] == "extra")
    };
    assert!(!has(&plain));
    assert!(has(&extra));
    assert_ne!(plain["context"], extra["context"]);
    let mut b = build(&root, "import renamed::extra;pub fn f()->u8{extra()}");
    assert!(b.check().err().unwrap().to_string().contains("L0513"));
    b.cargo.features = vec!["extra".into()];
    b.check().unwrap();
    let b = build(&root, "import renamed::count;pub fn f()->u8{count()}");
    b.generate().unwrap();
    assert!(
        b.is_current().unwrap(),
        "unchanged native build receipt must be current"
    );
    let provider = root.join("provider/src/lib.rs");
    let text = fs::read_to_string(&provider).unwrap();
    fs::write(
        &provider,
        text.replace("pub fn count()", "pub fn counter()"),
    )
    .unwrap();
    assert!(b.check().err().unwrap().to_string().contains("L0513"));
}
#[test]
#[doc = "spec: 1.30:8"]
fn logical_and_effect_contexts_cannot_treat_imports_as_math() {
    let root = fixture("effects");
    for promise in ["terminates", "no_panic", "no_alloc", "no_io"] {
        let error = build(
            &root,
            &format!("import renamed::tick;#[{promise}]fn f()->u8{{tick()}}"),
        )
        .check()
        .err()
        .unwrap()
        .to_string();
        assert!(error.contains(promise), "{error}");
    }
    let error = build(&root, "import renamed::tick;fn f()->@(tick()==1){_}")
        .check()
        .err()
        .unwrap()
        .to_string();
    assert!(!error.is_empty());
}
#[test]
#[doc = "spec: 1.30:9"]
fn upstream_versions_and_malformed_metadata_fail_closed() {
    let root = fixture("schema");
    let options = cargo::CargoOptions {
        manifest_path: Some(root.join("host/Cargo.toml")),
        offline: true,
        ..Default::default()
    };
    let ws = cargo::discover(&root.join("host"), &options)
        .unwrap()
        .unwrap();
    let mut extraction = imports::Extraction::new(&ws, &options).unwrap();
    let native = extraction
        .get(&ws, ws.packages[ws.host].dependencies["renamed"])
        .unwrap();
    let mut raw = serde_json::json!({"format_version":56,"includes_private":false,"root":native.interface.root.parse::<u64>().unwrap(),"index":native.interface.entities.iter().filter(|(_,e)|e.rustdoc.get("inner").is_some()).map(|(id,e)|(id.clone(),e.rustdoc.clone())).collect::<std::collections::BTreeMap<_,_>>(),"paths":native.interface.paths,"external_crates":native.interface.external_crates,"target":native.interface.target});
    Interface::read(
        &serde_json::to_vec(&raw).unwrap(),
        "test",
        serde_json::Value::Null,
    )
    .unwrap();
    raw["format_version"] = 999.into();
    let err = Interface::read(
        &serde_json::to_vec(&raw).unwrap(),
        "test",
        serde_json::Value::Null,
    )
    .unwrap_err();
    assert!(err.contains("999") && err.contains("56"), "{err}");
    raw["format_version"] = 56.into();
    raw["index"] = serde_json::Value::Null;
    assert!(
        Interface::read(
            &serde_json::to_vec(&raw).unwrap(),
            "test",
            serde_json::Value::Null
        )
        .unwrap_err()
        .contains("index")
    );
    assert!(
        Interface::read(b"not json", "test", serde_json::Value::Null)
            .unwrap_err()
            .contains("invalid JSON")
    );
}

#[test]
#[doc = "spec: 1.30:2, 1.30:7, 1.30:9"]
fn targets_macros_cli_and_failure_messages() {
    let root = fixture("cli");
    let cli = |path: &str, more: &[&str]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_locus"));
        cmd.args(["import", path, "--manifest-path"])
            .arg(root.join("host/Cargo.toml"))
            .arg("--offline")
            .args(more);
        cmd
    };
    let output = cli("native_macros", &["--json"]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["entities"]
            .as_object()
            .unwrap()
            .values()
            .any(|e| e["kind"] == "proc_macro" && e["name"] == "passthrough")
    );
    build(&root, "import native_macros; fn f()->u8{0}")
        .check()
        .unwrap();
    let version = Command::new("rustc").arg("-vV").output().unwrap();
    let version = String::from_utf8(version.stdout).unwrap();
    let host = version
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .unwrap();
    let output = cli("renamed", &["--json", "--target", host])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        json["context"]["native_flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == host)
    );
    for (more, phrase) in [
        (
            vec!["--target", "locus-nonexistent-target"],
            "locus-nonexistent-target",
        ),
        (vec!["--features", "absent_feature"], "absent_feature"),
    ] {
        let output = cli("renamed", &more).output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(phrase));
    }
    let output = cli("renamed", &[])
        .env("LOCUS_IMPORT_ACTIVE", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("recursive"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = cli("renamed", &[])
        .env("RUSTC_WRAPPER", "nonexistent-wrapper")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("RUSTC_WRAPPER"));
    let path = root.join("interface.json");
    fs::write(&path, "handwritten").unwrap();
    let output = cli("renamed", &["--out", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(&path).unwrap(), "handwritten");
    fs::remove_file(&path).unwrap();
    let output = cli("renamed", &["--out", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(&path).unwrap()).is_ok());
}

#[test]
#[doc = "spec: 1.30:1, 1.30:5"]
fn aliases_and_public_reexports_retain_native_identity() {
    let root = fixture("aliases");
    let checked = build(
        &root,
        "import renamed::echo as native_echo;pub use native_echo as echo;",
    )
    .check()
    .unwrap();
    let rust = locus::project::rust(checked).unwrap();
    fs::write(root.join("host/src/generated.rs"), rust).unwrap();
    fs::write(
        root.join("host/src/main.rs"),
        "mod generated {include!(\"generated.rs\");}fn main(){assert_eq!(generated::echo(9),9);}",
    )
    .unwrap();
    let output = Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("host/Cargo.toml"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let error = build(&root, "import renamed as same;import renamed as same;")
        .check()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("L0502"), "{error}");
}

#[test]
#[doc = "spec: 1.30:8"]
#[doc = "spec: 3.2:11"]
fn native_ir_cannot_return_proofs_or_consume_erased_information() {
    use locus::{
        erased::{self, EBlock, EExpr, EFn, EType},
        exec::{Block, ExecFn, Program, Promises, Tail},
        kernel::{Definitions, Term, Type},
    };
    for ty in [Type::Int, Type::Prop, Type::proof(Term::Bool(true))] {
        let mut program = Program::new(Definitions::default());
        let signature = Type::function_over(&[], &ty);
        let result = program.declare(ExecFn {
            signature,
            params: vec![],
            promises: Promises::default(),
            body: Block {
                stmts: vec![],
                tail: Tail::Foreign {
                    path: "::native::f".into(),
                    arguments: vec![],
                    result: ty,
                },
            },
        });
        assert!(result.is_err());
    }
    let mut module = erased::Module::default();
    let mut program = Program::new(Definitions::default());
    let reference = locus::typed::FnRef::Exec(
        program
            .declare(ExecFn {
                signature: Type::function_over(&[], &Type::U8),
                params: vec![],
                promises: Promises::default(),
                body: Block {
                    stmts: vec![],
                    tail: Tail::Value(Term::U8(0)),
                },
            })
            .unwrap(),
    );
    for (arguments, result) in [(vec![EExpr::Ghost], EType::Bool), (vec![], EType::Ghost)] {
        module.fns = vec![EFn {
            reference,
            name: "f".into(),
            constant: false,
            params: vec![],
            passing: vec![],
            result: result.clone(),
            body: EBlock {
                stmts: vec![],
                tail: Some(Box::new(EExpr::NativeCall {
                    path: "::native::f".into(),
                    arguments,
                    result,
                })),
            },
            owner: None,
            receiver: false,
        }];
        assert!(erased::check_module(&module).is_err());
    }
}

#[test]
#[doc = "spec: 1.30:7"]
fn a_dependency_import_uses_host_package_identity_not_its_local_alias() {
    let root = fixture("dependency_alias");
    let layer = root.join("layer");
    fs::create_dir_all(layer.join("src")).unwrap();
    fs::write(layer.join("Cargo.toml"),"[package]\nname=\"native_layer\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\ninner={package=\"native_provider\",path=\"../provider\"}\n[package.metadata.locus]\nlib=\"lib.lc\"\n").unwrap();
    fs::write(layer.join("src/lib.rs"), "pub use inner::echo;").unwrap();
    fs::write(layer.join("lib.lc"), "import inner::echo;pub use echo;").unwrap();
    let manifest = root.join("host/Cargo.toml");
    let content = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        format!(
            "{content}\n[dependencies.layer_alias]\npackage=\"native_layer\"\npath=\"../layer\"\n"
        ),
    )
    .unwrap();
    let checked = build(&root, "use layer_alias::echo;pub fn answer()->u8{echo(11)}")
        .check()
        .unwrap();
    let rust = locus::project::rust(checked).unwrap();
    assert!(!rust.contains("::inner::"));
    fs::write(root.join("host/src/generated.rs"), rust).unwrap();
    fs::write(
        root.join("host/src/main.rs"),
        "mod generated{include!(\"generated.rs\");}fn main(){assert_eq!(generated::answer(),11);}",
    )
    .unwrap();
    let output = Command::new("cargo")
        .args(["run", "--offline", "--quiet", "--manifest-path"])
        .arg(&manifest)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
