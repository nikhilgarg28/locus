use locus::project::{Build, cargo};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("packages_{name}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn workspace(name: &str) -> PathBuf {
    let root = scratch(name);
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packages"),
        &root,
    );
    for package in ["collections", "consumer"] {
        let path = root.join(package).join("Cargo.toml");
        let mut manifest = fs::read_to_string(&path).unwrap();
        manifest.push_str(&format!(
            "\n[build-dependencies]\nlocus = {{ path = {:?} }}\n",
            env!("CARGO_MANIFEST_DIR")
        ));
        fs::write(path, manifest).unwrap();
    }
    root
}
#[test]
#[doc = "spec: 1.28:9, 1.28:10, 1.28:11, 1.28:12"]
fn cargo_aliases_locus_theorems_and_runtime_nominal_identity() {
    let root = workspace("identity");
    let entry = root.join("consumer/locus/export.lc");
    let build = Build::new(&entry)
        .offline(true)
        .out_dir(root.join("out"))
        .name("local");
    let checked = build.check().unwrap_or_else(|e| panic!("{e}"));
    assert!(checked.checked.holes.iter().any(|h| h.solved));
    let generated = build.generate().unwrap_or_else(|e| panic!("{e}"));
    let source = fs::read_to_string(generated.rust).unwrap();
    assert!(source.contains("::verified::checked::Token"), "{source}");
    assert!(!source.contains("struct Token"));
    let result = Command::new("cargo")
        .args([
            "run",
            "--offline",
            "--quiet",
            "-p",
            "consumer",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("RUSTFLAGS", "-Dwarnings")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
#[doc = "spec: 1.28:15, 1.28:16"]
fn receipts_detect_modified_inputs_outputs_and_configuration() {
    let root = scratch("receipts");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='receipt_host'\nversion='0.1.0'\nedition='2024'\n[workspace]\n",
    )
    .unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "").unwrap();
    fs::write(root.join("export.lc"), "pub fn value() -> u8 { 7 }").unwrap();
    let build = Build::new(&root).offline(true).out_dir(root.join("out"));
    let first = build.generate().unwrap_or_else(|e| panic!("{e}"));
    let original = fs::read(&first.receipt).unwrap();
    assert!(build.is_current().unwrap());
    build.generate().unwrap();
    assert_eq!(fs::read(&first.receipt).unwrap(), original);
    fs::write(&first.rust, "tampered").unwrap();
    assert!(!build.is_current().unwrap());
    build.generate().unwrap();
    assert!(build.is_current().unwrap());
    fs::write(root.join("export.lc"), "pub fn value() -> u8 { 8 }").unwrap();
    assert!(!build.is_current().unwrap());
    build.generate().unwrap();
    assert!(build.is_current().unwrap());
    fs::remove_file(&first.rust).unwrap();
    assert!(!build.is_current().unwrap());
    build.generate().unwrap();
    fs::write(root.join("out/handwritten.rs"), "keep me").unwrap();
    let other = build.clone().name("handwritten");
    let err = other.generate().unwrap_err();
    assert!(err.to_string().contains("L0506"));
    assert_eq!(
        fs::read_to_string(root.join("out/handwritten.rs")).unwrap(),
        "keep me"
    );
}
#[test]
#[doc = "spec: 1.28:10"]
fn metadata_rejects_paths_outside_package_and_unknown_keys() {
    let root = workspace("bad_metadata");
    let manifest = root.join("collections/Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .replace("lib = \"locus/lib.lc\"", "lib = \"../secret.lc\"");
    fs::write(&manifest, text).unwrap();
    let error = cargo::discover(
        &root.join("consumer/locus/export.lc"),
        &cargo::CargoOptions {
            offline: true,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("inside its Cargo package"), "{error}");
}

#[test]
#[doc = "spec: 1.28:17"]
fn one_host_proof_lock_and_read_only_dependency_sources() {
    let root = workspace("proof_lock");
    let entry = root.join("consumer/locus/export.lc");
    let mut build = Build::new(&entry).offline(true);
    build.write_proofs = true;
    build.check().unwrap_or_else(|e| panic!("{e}"));
    let lock = root.join("consumer/Locus.lock");
    assert!(lock.is_file());
    assert!(!root.join("collections/Locus.lock").exists());
    assert!(!root.join("consumer/locus/Locus.lock").exists());
    let first = fs::read(&lock).unwrap();
    build.locked_proofs = true;
    build.check().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(fs::read(&lock).unwrap(), first);
    fs::remove_file(&lock).unwrap();
    let err = build.check().err().unwrap().to_string();
    assert!(err.contains("locked") || err.contains("proof"), "{err}");
}
#[test]
fn dependency_changes_invalidate_receipts_and_false_theorems_fail() {
    let root = workspace("dependency_change");
    let build = Build::new(root.join("consumer/locus/export.lc"))
        .offline(true)
        .out_dir(root.join("out"));
    build.generate().unwrap_or_else(|e| panic!("{e}"));
    assert!(build.is_current().unwrap());
    fs::write(
        root.join("collections/locus/theorems.lc"),
        "pub logic fn reflexive(n:Int)->@(n == n) { prove!(n == 1) }",
    )
    .unwrap();
    assert!(!build.is_current().unwrap());
    assert!(build.generate().is_err());
}
#[test]
fn dependency_runtime_items_without_rust_exports_fail_before_emission() {
    let root = workspace("hidden_runtime");
    fs::write(
        root.join("collections/locus/export.lc"),
        "pub use crate::runtime::Token;",
    )
    .unwrap();
    let err = Build::new(root.join("consumer/locus/export.lc"))
        .offline(true)
        .rust()
        .unwrap_err()
        .to_string();
    assert!(err.contains("L0504") && err.contains("increment"), "{err}");
}
#[test]
fn extracted_cargo_package_contains_sources_and_rebuilds_without_producer_outputs() {
    let producer = workspace("publication_producer");
    let manifest = producer.join("collections/Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .split("[build-dependencies]")
        .next()
        .unwrap()
        .to_owned();
    fs::write(&manifest, text).unwrap();
    // A distributable fixture uses an installed compiler executable, avoiding
    // reliance on a compiler-crate release that has not been published yet.
    fs::write(producer.join("collections/build.rs"),r#"fn main(){
        let compiler=std::env::var_os("LOCUS_COMPILER").expect("installed compiler");
        let out=std::env::var_os("OUT_DIR").unwrap();
        let status=std::process::Command::new(compiler).args(["build","locus/export.lc","--out-dir"]).arg(out).args(["--name","checked","--offline"]).status().unwrap();
        assert!(status.success());println!("cargo:rerun-if-changed=locus");
    }"#).unwrap();
    let package = Command::new("cargo")
        .args([
            "package",
            "--offline",
            "--no-verify",
            "--allow-dirty",
            "--manifest-path",
        ])
        .arg(&manifest)
        .env("CARGO_TARGET_DIR", producer.join("target"))
        .output()
        .unwrap();
    assert!(
        package.status.success(),
        "{}",
        String::from_utf8_lossy(&package.stderr)
    );
    let consumer = workspace("publication_consumer");
    fs::write(consumer.join("Cargo.toml"),"[workspace]\nmembers=['consumer']\nexclude=['unpacked/verified_collections-0.1.0']\nresolver='3'\n").unwrap();
    fs::create_dir(consumer.join("unpacked")).unwrap();
    let archive = producer.join("target/package/verified_collections-0.1.0.crate");
    assert!(
        Command::new("tar")
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(consumer.join("unpacked"))
            .status()
            .unwrap()
            .success()
    );
    let extracted = consumer.join("unpacked/verified_collections-0.1.0");
    assert!(extracted.join("locus/theorems.lc").is_file());
    assert!(!extracted.join("target").exists());
    let manifest = consumer.join("consumer/Cargo.toml");
    let text = fs::read_to_string(&manifest)
        .unwrap()
        .replace("../collections", "../unpacked/verified_collections-0.1.0");
    fs::write(manifest, text).unwrap();
    fs::remove_dir_all(&producer).unwrap();
    let out = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--offline",
            "-p",
            "consumer",
            "--manifest-path",
        ])
        .arg(consumer.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", consumer.join("target"))
        .env("LOCUS_COMPILER", env!("CARGO_BIN_EXE_locus"))
        .env("RUSTFLAGS", "-Dwarnings")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn unrelated_generation_targets_are_not_loaded_and_entries_need_no_library() {
    let root = workspace("independent_targets");
    let manifest = root.join("collections/Cargo.toml");
    let mut text = fs::read_to_string(&manifest).unwrap();
    text.push_str(
        "\n[[package.metadata.locus.targets]]\nname='unrelated'\nentry='locus/invalid.lc'\n",
    );
    fs::write(&manifest, text).unwrap();
    fs::write(
        root.join("collections/locus/invalid.lc"),
        "this is deliberately invalid",
    )
    .unwrap();
    Build::new(root.join("collections/locus/export.lc"))
        .offline(true)
        .check()
        .unwrap_or_else(|e| panic!("{e}"));
    let standalone = scratch("single_no_library");
    fs::write(standalone.join("entry.lc"), "pub fn answer()->u8 {42}").unwrap();
    // A source entry needs neither Cargo nor a specially named lib.lc. Use the
    // standalone loader API so the surrounding repository manifest is irrelevant.
    let source = project_rust(&standalone.join("entry.lc"));
    assert!(source.contains("answer"));
}
fn project_rust(path: &Path) -> String {
    locus::project::rust(
        locus::project::check(path, &Default::default()).unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn source_privacy_and_runtime_abi_fail_closed_across_packages() {
    let root = workspace("foreign_rejections");
    let source = root.join("consumer/locus/export.lc");
    let mut runtime = fs::read_to_string(root.join("collections/locus/runtime.lc")).unwrap();
    runtime.push_str("\npub(crate) fn hidden()->u8 {1}\n");
    fs::write(root.join("collections/locus/runtime.lc"), runtime).unwrap();
    for code in [
        "use verified::runtime::hidden; pub fn leak()->u8 {hidden()}",
        "use verified::runtime::Token; impl Token {pub fn invented(&self)->u8 {1}}",
        "use verified::runtime::Token; impl Model for Token { type Logic = Nat; logic fn model(&self)->Self::Logic {0} }",
        "use verified::runtime::Token; impl Model for (Token) { type Logic = Nat; logic fn model(&self)->Self::Logic {0} }",
    ] {
        fs::write(&source, code).unwrap();
        let err = Build::new(&source)
            .offline(true)
            .check()
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("L0503"), "{err}");
    }
    fs::write(
        &source,
        "use verified::runtime::optional; pub fn value()->Option<u8> {optional()}",
    )
    .unwrap();
    fs::write(
        root.join("collections/locus/runtime.lc"),
        "pub fn optional()->Option<u8> {Some(1)}",
    )
    .unwrap();
    fs::write(
        root.join("collections/locus/export.lc"),
        "pub use crate::runtime::optional;",
    )
    .unwrap();
    let err = Build::new(&source)
        .offline(true)
        .rust()
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("L0504") && err.contains("cross-package ABI"),
        "{err}"
    );
}
#[test]
fn cli_runs_module_aliases_and_checks_receipts() {
    let root = scratch("cli");
    fs::write(
        root.join("export.lc"),
        "mod inner {pub fn f()->u8 {42}} pub use inner::f as answer;",
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_locus");
    let out = Command::new(binary)
        .arg("run")
        .arg(&root)
        .args(["answer", "--offline"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"42\n");
    let out = Command::new(binary)
        .arg("build")
        .arg(&root)
        .arg("--out-dir")
        .arg(root.join("out"))
        .arg("--offline")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = Command::new(binary)
        .arg("build")
        .arg(&root)
        .arg("--out-dir")
        .arg(root.join("out"))
        .args(["--offline", "--check-receipt"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"current\n");
}

#[test]
#[doc = "spec: 1.28:12, 1.28:14"]
fn dependency_types_cannot_acquire_two_independent_rust_identities() {
    let root = workspace("ambiguous_identity");
    let manifest = root.join("collections/Cargo.toml");
    let mut metadata = fs::read_to_string(&manifest).unwrap();
    metadata
        .push_str("\n[[package.metadata.locus.targets]]\nname='second'\nentry='locus/second.lc'\n");
    fs::write(&manifest, metadata).unwrap();
    // The second interface reaches Token through a result, without naming it
    // in a re-export. Both generated Rust components would define their own copy.
    fs::write(
        root.join("collections/locus/second.lc"),
        "pub fn another()->crate::runtime::Token {crate::runtime::Token::new(1)}",
    )
    .unwrap();
    let error = Build::new(root.join("consumer/locus/export.lc"))
        .offline(true)
        .rust()
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("L0504") && error.contains("independent targets") && error.contains("Token"),
        "{error}"
    );
    // Multiple names for the same type inside one target preserve identity.
    let text = fs::read_to_string(&manifest).unwrap();
    fs::write(
        &manifest,
        text.split("\n[[package.metadata.locus.targets]]\nname='second'")
            .next()
            .unwrap(),
    )
    .unwrap();
    let export = root.join("collections/locus/export.lc");
    let text =
        fs::read_to_string(&export).unwrap() + "\npub use crate::runtime::Token as TokenAlias;\n";
    fs::write(export, text).unwrap();
    Build::new(root.join("consumer/locus/export.lc"))
        .offline(true)
        .rust()
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
#[doc = "spec: 1.28:12"]
fn generic_dependency_runtime_bodies_are_not_copied_but_logic_can_erase() {
    let root = workspace("generic_dependency");
    fs::write(
        root.join("collections/locus/runtime.lc"),
        "pub fn identity<T>(x:T)->T{x} pub logic fn logical<T:Logical>(x:T)->T{x}",
    )
    .unwrap();
    fs::write(root.join("collections/locus/export.lc"), "").unwrap();
    let entry = root.join("consumer/locus/export.lc");
    fs::write(
        &entry,
        "pub fn answer()->u8 {verified::runtime::identity::<u8>(7)}",
    )
    .unwrap();
    let error = Build::new(&entry)
        .offline(true)
        .rust()
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("L0504") && error.contains("generic runtime dependency"),
        "{error}"
    );
    fs::write(
        &entry,
        "pub fn answer()->u8 {let thought = verified::runtime::logical::<Int>(1); 7}",
    )
    .unwrap();
    Build::new(&entry)
        .offline(true)
        .rust()
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
#[doc = "spec: 1.28:12, 1.28:18"]
fn dependency_proof_results_do_not_silently_call_a_projected_abi() {
    let root = workspace("proof_result_abi");
    let source = root.join("collections/locus/runtime.lc");
    let current = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        current.replace(
            "pub fn increment(value: u8) -> u8 { value.wrapping_add(1) }",
            "pub fn increment(value: u8) -> (u8, @(1 == 1)) { (value.wrapping_add(1), _) }",
        ),
    )
    .unwrap();
    let producer = Build::new(root.join("collections/locus/export.lc"))
        .offline(true)
        .rust()
        .unwrap();
    assert!(producer.contains("__locus_with_proofs_"));
    fs::write(
        root.join("consumer/locus/export.lc"),
        "pub fn run()->u8 { let (n, proof) = verified::runtime::increment(1); n }",
    )
    .unwrap();
    let error = Build::new(root.join("consumer/locus/export.lc"))
        .offline(true)
        .rust()
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("L0504") && error.contains("cross-package runtime proof interfaces"),
        "{error}"
    );
}
