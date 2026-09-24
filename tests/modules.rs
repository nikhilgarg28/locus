use locus::{elab, project};
use std::path::{Path, PathBuf};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/modules")
        .join(name)
}
fn check(path: &Path) -> Result<(), String> {
    let loaded = project::load(path).map_err(|e| e.to_string())?;
    let options = elab::Options {
        module_access: Some(std::sync::Arc::new(loaded.graph.access.clone())),
        ..Default::default()
    };
    let checked = elab::elaborate_with_options(
        loaded.sources.get(loaded.bundle.file),
        &loaded.program,
        &options,
    );
    if !checked.is_success() {
        return Err(checked
            .diagnostics
            .iter()
            .map(|d| loaded.bundle.diagnostic(d).render(&loaded.sources, false))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    Ok(())
}
#[test]
#[doc = "spec: 1.28:2, 1.28:3"]
fn directory_and_file_entries_load_only_declared_modules() {
    let root = fixture("basic");
    check(&root).unwrap();
    check(&root.join("export.lc")).unwrap();
    let loaded = project::load(&root).unwrap();
    assert_eq!(loaded.inputs.len(), 4);
    assert!(!loaded.inputs.keys().any(|p| p.ends_with("ignored.lc")));
    assert!(loaded.graph.exports(0).iter().any(|e| e.path == ["add"]));
}
#[test]
fn missing_child_is_reported_at_its_declaration() {
    let error = project::load(&fixture("missing")).err().unwrap();
    assert_eq!(error.diagnostics[0].code, "L0501");
    let rendered = error.to_string();
    assert!(rendered.contains("export.lc:1"), "{rendered}");
    assert!(rendered.contains("absent.lc"));
}
#[test]
fn private_items_cannot_be_named_from_the_parent() {
    let error = project::load(&fixture("private")).err().unwrap();
    assert_eq!(error.diagnostics[0].code, "L0503");
    assert!(error.to_string().contains("child.lc"));
}
fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("modules_{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn generate(path: &Path) -> Result<String, String> {
    project::rust(project::check(path, &elab::Options::default()).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn rustc(dir: &Path, body: &str) -> std::process::Output {
    std::fs::write(dir.join("main.rs"), body).unwrap();
    std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("run"))
        .output()
        .unwrap()
}
#[test]
#[doc = "spec: 1.28:14, 1.19:1"]
fn rust_export_reexports_execute_and_private_helpers_stay_private() {
    let dir = scratch("rust");
    let rust = generate(&fixture("basic")).unwrap();
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let good = rustc(
        &dir,
        "include!(\"generated.rs\"); fn main(){assert_eq!(answer(),42);assert_eq!(Counter::new(7).get(),7);}",
    );
    assert!(
        good.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&good.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("run"))
            .status()
            .unwrap()
            .success()
    );
    let hidden = rust
        .lines()
        .find(|l| l.contains("fn __locus_") && l.contains("_identity("))
        .unwrap()
        .split("fn ")
        .nth(1)
        .unwrap()
        .split('(')
        .next()
        .unwrap();
    let bad = rustc(
        &dir,
        &format!("include!(\"generated.rs\"); fn main(){{__locus_impl::{hidden}(1);}}"),
    );
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("E0603"));
}
#[test]
#[doc = "spec: 1.28:13, 1.17:1"]
fn export_rejects_logic_in_results_callbacks_and_reachable_methods() {
    let dir = scratch("leaks");
    let file = dir.join("export.lc");
    for source in [
        "pub fn claim() -> @(1 == 1) { prove!(1 == 1) }",
        "pub fn pair() -> (u8, @(1 == 1)) { (1, prove!(1 == 1)) }",
        "pub struct Secret { data: u8 } impl Secret { pub fn claim(&self) -> @(1 == 1) { prove!(1 == 1) } }",
        "pub enum Answer { Data(u8), Proof(@(1 == 1)) }",
        "pub struct Unsafe { pub data:u8, proof:@(data as Int == 1) }",
    ] {
        std::fs::write(&file, source).unwrap();
        let err = generate(&file).unwrap_err();
        assert!(err.contains("L0504"), "{source}\n{err}");
    }
}
#[test]
fn private_proof_fields_and_proof_taking_helpers_cannot_be_reached_from_rust() {
    let dir = scratch("invariant");
    let file = dir.join("export.lc");
    std::fs::write(&file,"mod value { pub struct One { value:u8, proof:@(value as Int == 1) } impl One { pub fn new() -> One { One {value:1, proof:prove!(1 == 1)} } pub fn value(&self)->u8 {self.value} pub(crate) fn trusted(value:u8, proof:@(value as Int == 1))->One {One {value,proof}} } } pub use value::One;").unwrap();
    let rust = generate(&file).unwrap();
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let good = rustc(
        &dir,
        "include!(\"generated.rs\");fn main(){assert_eq!(One::new().value(),1);}",
    );
    assert!(
        good.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&good.stderr)
    );
    for body in [
        "let mut x=One::new(); x.value=2;",
        "let _ = One::trusted;",
        "let _ = __locus_impl::Erased;",
    ] {
        let bad = rustc(
            &dir,
            &format!("include!(\"generated.rs\"); fn main(){{{body}}}"),
        );
        assert!(!bad.status.success(), "{body}");
    }
}

#[test]
#[doc = "spec: 1.28:6"]
fn privacy_checks_fields_methods_references_and_logical_observations() {
    let root = scratch("privacy");
    let entry = root.join("export.lc");
    let setup = "mod child { pub struct Secret { value:u8 } impl Secret { pub fn new()->Secret {Secret {value:1}} fn hidden(&self)->u8 {self.value} } } ";
    for body in [
        "let s=child::Secret::new();s.value",
        "let s=child::Secret::new();s.hidden()",
        "let s=child::Secret {value:1};0",
        "let s=child::Secret::new();let p=prove!(model!(s.value) == 1);0",
    ] {
        fs_write(&entry, &format!("{setup} pub fn attack()->u8 {{{body}}}"));
        let error = project::check(&entry, &Default::default())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("L0503"), "{body}\n{error}");
    }
}
fn fs_write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}
#[test]
#[doc = "spec: 1.28:5, 1.28:6"]
fn resolution_supports_shadowing_forward_aliases_and_restricted_ancestors() {
    let root = scratch("paths");
    let entry = root.join("export.lc");
    fs_write(
        &entry,
        r#"
mod internal {
    pub mod nested {
        pub(in crate::internal) fn secret()->u8 {7}
        pub fn value()->u8 {secret()}
    }
    pub fn value()->u8 {nested::secret()}
}
use alias as later;
use internal::{self as implementation, value as alias};
pub use implementation::nested::value;
pub fn result()->u8 { let alias:u8=1; later().wrapping_add(alias) }
"#,
    );
    check(&entry).unwrap();
    let rust = generate(&entry).unwrap();
    fs_write(&root.join("generated.rs"), &rust);
    let out = rustc(
        &root,
        "include!(\"generated.rs\");fn main(){assert_eq!(result(),8);assert_eq!(value(),7);}",
    );
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    fs_write(&entry, "mod a {pub(in crate::missing) fn bad()->u8 {1}} ");
    assert!(
        project::load(&entry)
            .err()
            .unwrap()
            .to_string()
            .contains("L0503")
    );
}
#[test]
#[doc = "spec: 1.28:3, 1.28:8"]
fn module_cycles_ambiguity_unused_loaded_errors_and_reserved_names_fail() {
    let root = scratch("loader_errors");
    let entry = root.join("export.lc");
    fs_write(&entry, "mod a;");
    fs_write(&root.join("a.lc"), "fn value()->u8 {1}");
    std::fs::create_dir(root.join("a")).unwrap();
    fs_write(&root.join("a/mod.lc"), "fn value()->u8 {1}");
    assert!(
        project::load(&entry)
            .err()
            .unwrap()
            .to_string()
            .contains("ambiguous module")
    );
    std::fs::remove_file(root.join("a/mod.lc")).unwrap();
    fs_write(&root.join("a.lc"), "fn unused()->u8 {true}");
    assert!(project::check(&entry, &Default::default()).is_err());
    fs_write(
        &entry,
        "pub fn f()->u8 {let __locus_m0_n0_f=1;__locus_m0_n0_f}",
    );
    assert!(project::load(&entry).is_err());
    fs_write(&entry, "use b as a; use a as b;");
    assert!(
        project::load(&entry)
            .err()
            .unwrap()
            .to_string()
            .contains("cyclic re-exports")
    );
    #[cfg(unix)]
    {
        fs_write(&entry, "mod cycle;");
        std::os::unix::fs::symlink(&entry, root.join("cycle.lc")).unwrap();
        assert!(
            project::load(&entry)
                .err()
                .unwrap()
                .to_string()
                .contains("cyclic module file")
        );
    }
}

#[test]
#[doc = "spec: 1.28:5"]
fn qualified_monomorphic_types_work_without_import_aliases() {
    let dir = scratch("qualified_types");
    let file = dir.join("export.lc");
    std::fs::write(
        &file,
        r#"
        mod types { pub struct Token { pub value: u8 } }
        pub fn token(value: u8) -> crate::types::Token {
            crate::types::Token { value }
        }
        pub fn unpack(value: (types::Token,)) -> self::types::Token { value.0 }
    "#,
    )
    .unwrap();
    let rust = generate(&file).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let result = rustc(
        &dir,
        "include!(\"generated.rs\"); fn main(){assert_eq!(unpack((token(7),)).value,7);}",
    );
    assert!(
        result.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("run"))
            .status()
            .unwrap()
            .success()
    );
}

#[test]
#[doc = "spec: 1.28:5, 1.28:6, 1.92:5"]
fn derived_models_have_module_identity_and_preserve_field_privacy() {
    let root = scratch("derived_models");
    let entry = root.join("export.lc");
    fs_write(
        &root.join("point.lc"),
        r#"
#[derive(Model)] pub struct Point { pub x: u8, hidden: i32 }
impl Point { pub fn new() -> Point { Point { x: 3, hidden: 4 } } }
pub logic fn inspect(point: &Point) -> PointModel { model!(point) }
"#,
    );
    fs_write(
        &entry,
        r#"
mod point;
use point::{Point, PointModel, inspect};
logic fn nonnegative(value: PointModel) -> @(value.x >= 0) { _ }
pub fn run() -> u8 {
    let point = Point::new();
    let direct: PointModel = model!(point);
    let direct_fact = nonnegative(direct);
    let value: point::PointModel = inspect(&point);
    let fact = nonnegative(value);
    point.x
}
"#,
    );
    let rust = generate(&entry).unwrap();
    fs_write(&root.join("generated.rs"), &rust);
    let out = rustc(
        &root,
        "include!(\"generated.rs\");fn main(){assert_eq!(run(),3);}",
    );
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    for body in [
        "let p = Point::new(); let h = prove!(p.hidden == 4);",
        "let p = Point::new(); let h = model!(p.hidden);",
    ] {
        fs_write(
            &entry,
            &format!("mod point; use point::Point; fn attack() -> () {{ {body} }}"),
        );
        assert!(check(&entry).unwrap_err().contains("L0503"), "{body}");
    }
    fs_write(&entry, "mod point; pub use point::PointModel;");
    assert!(generate(&entry).unwrap_err().contains("L0504"));
}

#[test]
#[doc = "spec: 1.28:6, 1.28:13, 1.92:15"]
fn associated_constants_keep_module_privacy_and_export_checked_values() {
    let root = scratch("associated_constants");
    let entry = root.join("export.lc");
    let setup = r#"mod constants {
        pub struct Limits {}
        impl Limits {
            pub const NEXT: u8 = Self::BASE + 1;
            const BASE: u8 = 41;
            pub(crate) const INTERNAL: Nat = 9;
        }
    }"#;
    fs_write(
        &entry,
        &format!("{setup} pub use constants::Limits; pub fn value()->u8 {{ Limits::NEXT }}"),
    );
    let rust = generate(&entry).unwrap();
    fs_write(&root.join("generated.rs"), &rust);
    let out = rustc(
        &root,
        "include!(\"generated.rs\");fn main(){assert_eq!(value(),42);assert_eq!(Limits::NEXT,42);}",
    );
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        std::process::Command::new(root.join("run"))
            .status()
            .unwrap()
            .success()
    );
    let bad = rustc(
        &root,
        "include!(\"generated.rs\");fn main(){let _=Limits::BASE;}",
    );
    assert!(!bad.status.success());
    fs_write(
        &entry,
        &format!("{setup} fn attack()->u8 {{constants::Limits::BASE}}"),
    );
    assert!(check(&entry).unwrap_err().contains("L0503"));
    fs_write(
        &entry,
        "pub struct Limits {} impl Limits { pub const LOGICAL: Nat = 1; }",
    );
    assert!(generate(&entry).unwrap_err().contains("L0504"));
}

#[test]
#[doc = "spec: 1.28:5, 1.92:1, 1.92:4"]
fn explicit_models_and_observations_work_across_files() {
    let root = scratch("explicit_models");
    let entry = root.join("export.lc");
    fs_write(
        &root.join("point.lc"),
        r#"
pub struct Point { x: u8 }
#[derive(Logical)] pub struct Position { pub horizontal: Nat }
impl Model for Point {
    type Logic = Position;
    logic fn model(&self) -> Self::Logic { Position { horizontal: model!(self.x) } }
}
impl Point { pub fn new() -> Point { Point { x: 3 } } }
"#,
    );
    fs_write(
        &entry,
        r#"
mod point;
use point::{Point, Position};
fn read(point: &Point) -> Position { model!(point) }
fn nonnegative(point: &Point) -> @(point.horizontal >= 0) { _ }
pub fn run() -> u8 { let point = Point::new(); let value = read(&point); let h = nonnegative(&point); 3 }
"#,
    );
    generate(&entry).unwrap();
}

#[test]
#[doc = "spec: 1.28:14, 1.27:3, 1.92:8"]
fn checked_arithmetic_survives_module_wrappers_and_proof_erasure() {
    let root = scratch("arithmetic");
    let entry = root.join("export.lc");
    fs_write(
        &root.join("arithmetic.lc"),
        r#"
pub fn increment(n: u8) -> (out: u8, @(out == n + 1)) {
    let out = n + 1;
    (out, _)
}
pub fn guarded(n: u8) -> u8 { if n < 255 { n + 1 } else { 0 } }
"#,
    );
    fs_write(
        &entry,
        r#"
mod arithmetic;
pub use arithmetic::guarded;
pub fn increment(n: u8) -> u8 { let (out, proof) = arithmetic::increment(n); out }
"#,
    );
    let rust = generate(&entry).unwrap();
    assert!(rust.contains("checked_add"), "{rust}");
    fs_write(&root.join("generated.rs"), &rust);
    fs_write(
        &root.join("main.rs"),
        r#"
include!("generated.rs");
fn main() {
    assert_eq!(increment(41), 42);
    assert_eq!(guarded(254), 255);
    assert_eq!(guarded(255), 0);
    assert!(std::panic::catch_unwind(|| increment(255)).is_err());
}
"#,
    );
    for mode in ["yes", "no"] {
        let out = std::process::Command::new("rustc")
            .args([
                "--edition=2024",
                "-Dwarnings",
                "-C",
                &format!("overflow-checks={mode}"),
            ])
            .arg(root.join("main.rs"))
            .arg("-o")
            .arg(root.join("run"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}\n{rust}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            std::process::Command::new(root.join("run"))
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
