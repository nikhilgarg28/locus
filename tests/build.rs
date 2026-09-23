//! The export boundary, tested from the Rust side (Target language: What is
//! generated; O2). `locus build` writes a crate from a Locus file; rustc
//! compiles it; and hand-written Rust against it is compiled in turn, to
//! see what it can reach. What Rust can reach is what was marked plain
//! `pub`, and none of it takes evidence: a marker obtained honestly, from
//! a function that returns one, cannot be replayed into any function that
//! wants evidence, because no such function is exported. That is what the
//! guarantee rests on; that a marker cannot be made from nothing is
//! checked too, and is no part of it.
//!
//! Each refusal is asserted on rustc's error code, so that the test says
//! exactly which wall was hit.
//!
//! The protected type of Target examples, `Percent`, is the same boundary
//! with an `impl` block (O4): `checked` is `pub` and answers with an enum
//! (`Option`, emitted as a concrete monomorphic enum), `new` takes evidence
//! and is `pub(super)`, and the
//! fields are private, so a Rust caller can hold a `Percent`, cannot make
//! one, and can only change one through its methods. The panic test is
//! case 11 of How mutation is checked: a method that completes one valid
//! replacement through `&mut self` and then panics leaves the caller the
//! new value, valid, and never a broken one.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A lock whose evidence-taking step is `pub(crate)`, a plain `pub`
/// function, a `pub` struct with a private field, and a `pub` function
/// that returns evidence, which is harmless.
const LOCK: &str = r#"
#[derive(Clone, Copy, Debug)]
pub struct Lock {
    pub failures: u8,
    open: bool,
}

prop within_limit(failures: Int) {
    Bounds => { prop!(failures <= 3) }
}

pub fn locked() -> Lock {
    Lock { failures: 0, open: false }
}

pub fn bounded_zero() -> @within_limit(0) {
    within_limit::Bounds @ prove!(0 <= 3)
}

pub(crate) fn step(lock: Lock, bounded: @within_limit(lock.failures as Int)) -> (next: Lock, @within_limit(next.failures as Int)) {
    (lock, bounded)
}

pub fn total(a: u8, b: u8) -> u32 {
    (a as u32) + (b as u32)
}
"#;

/// The same, with `step` plain `pub`: the marker replay attack as Locus
/// source, which Locus refuses.
const LOCK_EXPORTING_STEP: &str = r#"
#[derive(Clone, Copy, Debug)]
pub struct Lock {
    pub failures: u8,
    open: bool,
}

prop within_limit(failures: Int) {
    Bounds => { prop!(failures <= 3) }
}

pub fn bounded_zero() -> @within_limit(0) {
    within_limit::Bounds @ prove!(0 <= 3)
}

pub fn step(lock: Lock, bounded: @within_limit(lock.failures as Int)) -> (next: Lock, @within_limit(next.failures as Int)) {
    (lock, bounded)
}
"#;

/// The protected type, as `tests/corpus/target/percent.lc` has it.
const PERCENT: &str = concat!(
    include_str!("corpus/target/percent.lc"),
    r#"
// Additional mutation methods exercise the same protected invariant from Rust.
impl Percent {
    #[terminates] #[no_panic] #[no_io]
    pub fn value(&self) -> u32 { self.value }

    #[no_io]
    pub fn set(&mut self, value: u32) -> bool {
        let checked: Option<Percent> = Percent::checked(value);
        match checked {
            Option::Some(next) => { *self = next; true }
            Option::None => false,
        }
    }

    #[no_io]
    pub fn set_twice(&mut self, a: u32, b: u32) -> () {
        assert!(a <= 100);
        *self = Percent { value: a, in_range: prove!((a as Int) <= 100) };
        assert!(b <= 100);
        *self = Self { value: b, in_range: prove!((b as Int) <= 100) };
    }
}
"#
);

fn workspace(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("build_{name}"));
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn rustc() -> Command {
    Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
}

/// Runs `locus build` on one Locus file, into `<workspace>/generated`, and
/// returns the crate's directory.
fn build_crate(name: &str, source: &str) -> PathBuf {
    build_crate_from(name, "lock.lc", source)
}

/// `build_crate` with the Locus file's name, which names the module.
fn build_crate_from(name: &str, file: &str, source: &str) -> PathBuf {
    let workspace = workspace(name);
    let file = workspace.join(file);
    std::fs::write(&file, source).unwrap();
    let out = workspace.join("generated");
    let mut command = Command::new(env!("CARGO_BIN_EXE_locus"));
    command.arg("build").arg(&file);
    for feature in [
        locus::preview::Feature::LogicalData,
        locus::preview::Feature::HeapViews,
    ] {
        if feature.status() == locus::preview::Status::Preview {
            command.args(["--preview", feature.name()]);
        }
    }
    let output = command
        .arg("--out")
        .arg(&out)
        .args(["--name", "generated"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "locus build failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    out
}

/// Compiles the generated crate as a library, under `-D warnings`, and
/// returns the rlib.
fn compile_generated(directory: &Path) -> PathBuf {
    let rlib = directory.join("libgenerated.rlib");
    let output = rustc()
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--crate-name",
            "generated",
            "-D",
            "warnings",
            "-o",
        ])
        .arg(&rlib)
        .arg(directory.join("src").join("lib.rs"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "rustc rejected the generated crate:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    rlib
}

/// Compiles hand-written Rust against the generated crate: the binary, or
/// what rustc wrote when it refused.
fn compile_caller(
    directory: &Path,
    name: &str,
    source: &str,
    rlib: &Path,
) -> Result<PathBuf, String> {
    let path = directory.join(format!("{name}.rs"));
    std::fs::write(&path, source).unwrap();
    let binary = directory.join(name);
    let output = rustc()
        .args(["--edition", "2024", "--crate-type", "bin", "-o"])
        .arg(&binary)
        .arg("--extern")
        .arg(format!("generated={}", rlib.display()))
        .arg(&path)
        .output()
        .unwrap();
    if output.status.success() {
        Ok(binary)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

/// The error codes in what rustc wrote, `E0603` and so on, in order.
fn error_codes(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("error[")?;
            let end = rest.find(']')?;
            Some(rest[..end].to_string())
        })
        .collect()
}

/// The generated crate and its rlib, built once per test.
fn generated(name: &str) -> (PathBuf, PathBuf) {
    let directory = build_crate(name, LOCK);
    let rlib = compile_generated(&directory);
    (directory, rlib)
}

/// The generated crate of the protected type, and its rlib.
fn generated_percent(name: &str) -> (PathBuf, PathBuf) {
    let directory = build_crate_from(name, "percent.lc", PERCENT);
    let rlib = compile_generated(&directory);
    (directory, rlib)
}

/// Runs a compiled caller and returns what it printed, after asserting
/// that it exited well.
fn run(binary: PathBuf) -> String {
    let output = Command::new(binary).output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[doc = "spec: 1.17:1"]
fn a_plain_pub_function_is_called_from_rust_and_runs() {
    let (directory, rlib) = generated("total");
    let binary = compile_caller(
        &directory,
        "calls_total",
        "fn main() {\n    let lock = generated::lock::locked();\n    println!(\"{} {}\", generated::lock::total(2, 3), lock.failures);\n}\n",
        &rlib,
    )
    .unwrap_or_else(|stderr| panic!("rustc refused a call to a `pub` function:\n{stderr}"));
    let output = Command::new(binary).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "5 0\n");
}

#[test]
#[doc = "spec: 1.17:1"]
fn an_evidence_taking_function_is_not_reachable_from_rust() {
    // `step` is `pub(crate)`: a function under the generated root can call
    // it, and a Rust caller in another crate cannot name it.
    let (directory, rlib) = generated("step");
    let stderr = compile_caller(
        &directory,
        "calls_step",
        "fn main() {\n    let lock = generated::lock::locked();\n    let bounded = generated::lock::bounded_zero();\n    let _ = generated::lock::step(lock, bounded);\n}\n",
        &rlib,
    )
    .expect_err("a `pub(crate)` function is not reachable from another crate");
    assert_eq!(error_codes(&stderr), ["E0603"], "{stderr}");
    assert!(stderr.contains("`step` is private"), "{stderr}");
}

#[test]
#[doc = "spec: 1.17:1"]
fn the_marker_replay_attack_does_not_compile() {
    // The attack: obtain a marker honestly, from `bounded_zero`, and hand
    // it to a function that wants evidence about a lock with three
    // failures. The Rust side is refused because `step` is not exported;
    // the Locus side, `pub fn step`, is refused by Locus.
    let (directory, rlib) = generated("replay");
    let stderr = compile_caller(
        &directory,
        "replays",
        "fn main() {\n    let mut lock = generated::lock::locked();\n    lock.failures = 200;\n    let replayed = generated::lock::bounded_zero();\n    let (next, _) = generated::lock::step(lock, replayed);\n    println!(\"{}\", next.failures);\n}\n",
        &rlib,
    )
    .expect_err("the replay is refused");
    assert_eq!(error_codes(&stderr), ["E0603"], "{stderr}");

    let workspace = workspace("replay_locus");
    let file = workspace.join("lock.lc");
    std::fs::write(&file, LOCK_EXPORTING_STEP).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("build")
        .arg(&file)
        .arg("--out")
        .arg(workspace.join("generated"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("error[L0244]"), "{stderr}");
    assert!(
        stderr.contains("`step` takes evidence and cannot be `pub`"),
        "{stderr}"
    );
    assert!(stderr.contains("make it `pub(crate)`"), "{stderr}");
    // Nothing was written.
    assert!(!workspace.join("generated").exists());
}

#[test]
#[doc = "spec: 1.17:1"]
fn the_marker_cannot_be_made_by_a_rust_caller() {
    let (directory, rlib) = generated("marker");
    // As a value: the constant of that name is private to the root.
    let stderr = compile_caller(
        &directory,
        "names_marker",
        "fn main() {\n    let _: generated::Erased = generated::Erased;\n}\n",
        &rlib,
    )
    .expect_err("the marker's constant is private");
    assert_eq!(error_codes(&stderr), ["E0603"], "{stderr}");
    // As a struct literal: its one field is private.
    let stderr = compile_caller(
        &directory,
        "builds_marker",
        "fn main() {\n    let _ = generated::Erased { _private: () };\n}\n",
        &rlib,
    )
    .expect_err("the marker's field is private");
    assert_eq!(error_codes(&stderr), ["E0451"], "{stderr}");
    // As a call: the private constant is what the name finds, and it is no
    // function.
    let stderr = compile_caller(
        &directory,
        "calls_marker",
        "fn main() {\n    let _ = generated::Erased(());\n}\n",
        &rlib,
    )
    .expect_err("the marker has no constructor function");
    assert_eq!(error_codes(&stderr), ["E0603", "E0618"], "{stderr}");
    // The type is nameable, and a marker can be held once obtained.
    let binary = compile_caller(
        &directory,
        "holds_marker",
        "fn main() {\n    let held: generated::Erased = generated::lock::bounded_zero();\n    println!(\"{held:?}\");\n}\n",
        &rlib,
    )
    .unwrap_or_else(|stderr| panic!("a marker can be held:\n{stderr}"));
    let output = Command::new(binary).output().unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Erased\n");
}

#[test]
#[doc = "spec: 1.17:1"]
fn a_struct_with_a_private_field_cannot_be_built_or_opened_from_rust() {
    let (directory, rlib) = generated("private_field");
    let stderr = compile_caller(
        &directory,
        "builds_lock",
        "fn main() {\n    let _ = generated::lock::Lock { failures: 1, open: false };\n}\n",
        &rlib,
    )
    .expect_err("a private field cannot be given");
    assert_eq!(error_codes(&stderr), ["E0451"], "{stderr}");
    let stderr = compile_caller(
        &directory,
        "reads_lock",
        "fn main() {\n    let lock = generated::lock::locked();\n    println!(\"{}\", lock.open);\n}\n",
        &rlib,
    )
    .expect_err("a private field cannot be read");
    assert_eq!(error_codes(&stderr), ["E0616"], "{stderr}");
}

#[test]
fn the_generated_crate_has_the_plain_layout_and_is_the_same_twice() {
    let directory = build_crate("layout", LOCK);
    let read = |path: &str| std::fs::read_to_string(directory.join(path)).unwrap();
    let manifest = read("Cargo.toml");
    assert!(manifest.contains("name = \"generated\""), "{manifest}");
    assert!(manifest.contains("edition = \"2024\""), "{manifest}");
    let root = read("src/lib.rs");
    assert!(root.contains("pub struct Erased {"), "{root}");
    assert!(
        root.contains("const Erased: Erased = Erased { _private: () };"),
        "{root}"
    );
    assert!(root.contains("pub mod lock;"), "{root}");
    let module = read("src/lock.rs");
    assert!(
        module.starts_with("// Generated by Locus. Do not edit.\n"),
        "{module}"
    );
    assert!(module.contains("use crate::Erased;"), "{module}");
    assert!(!module.contains("struct Erased"), "{module}");
    assert!(
        module.contains("\npub struct Lock {\n    pub failures: u8,")
            && module.contains("    open: bool,\n}"),
        "{module}"
    );
    assert!(
        module.contains("\npub(crate) fn step(lock: Lock, _bounded: Erased) -> (Lock, Erased) {"),
        "{module}"
    );
    assert!(
        module.contains("\npub fn total(a: u8, b: u8) -> u32 {"),
        "{module}"
    );
    // A function of the logic has no runtime form, and is not emitted.
    assert!(!module.contains("within_limit"), "{module}");
    let again = build_crate("layout", LOCK);
    assert_eq!(again, directory);
    for path in ["Cargo.toml", "src/lib.rs", "src/lock.rs"] {
        assert_eq!(
            read(path),
            std::fs::read_to_string(again.join(path)).unwrap(),
            "{path}"
        );
    }
}

#[test]
fn checked_is_called_from_rust_and_answers_with_the_enum() {
    let (directory, rlib) = generated_percent("percent_checked");
    let binary = compile_caller(
        &directory,
        "calls_checked",
        "use generated::percent::{Percent, __LocusOption0};\nfn main() {\n    for value in [42, 100, 101] {\n        let checked = Percent::checked(value);
        match checked {\n            __LocusOption0::Some(percent) => println!(\"valid {}\", percent.value()),\n            __LocusOption0::None => println!(\"invalid\"),\n        }\n    }\n}\n",
        &rlib,
    )
    .unwrap_or_else(|stderr| panic!("rustc refused a call to `checked`:\n{stderr}"));
    assert_eq!(run(binary), "valid 42\nvalid 100\ninvalid\n");
}

#[test]
fn new_is_not_visible_from_rust() {
    // `new` takes evidence and is `pub(crate)`: an associated function
    // that is private to the generated crate, which rustc reports as
    // E0624, the code for a private associated function, where a private
    // free function gets E0603. The evidence argument is `todo!()`, so
    // that the only wall hit is the one the test is about.
    let (directory, rlib) = generated_percent("percent_new");
    let stderr = compile_caller(
        &directory,
        "calls_new",
        "fn main() {\n    let percent = generated::percent::Percent::new(5, todo!());\n    println!(\"{}\", percent.value());\n}\n",
        &rlib,
    )
    .expect_err("a `pub(crate)` associated function is not reachable from another crate");
    assert_eq!(error_codes(&stderr), ["E0624"], "{stderr}");
    assert!(
        stderr.contains("associated function `new` is private"),
        "{stderr}"
    );
}

#[test]
fn percent_cannot_be_built_or_opened_from_rust() {
    let (directory, rlib) = generated_percent("percent_fields");
    // As a struct literal: both fields are private, and the marker's
    // constant is private to the root besides.
    let stderr = compile_caller(
        &directory,
        "builds_percent",
        "fn main() {\n    let percent = generated::percent::Percent { value: 200, in_range: todo!() };\n    println!(\"{}\", percent.value());\n}\n",
        &rlib,
    )
    .expect_err("a private field cannot be given");
    assert_eq!(error_codes(&stderr), ["E0451"], "{stderr}");
    // As a field read: the value is reached through `value()` alone.
    let stderr = compile_caller(
        &directory,
        "reads_percent",
        "fn main() {\n    if let generated::percent::__LocusOption0::Some(percent) = generated::percent::Percent::checked(5) {\n        println!(\"{}\", percent.value);\n    }\n}\n",
        &rlib,
    )
    .expect_err("a private field cannot be read");
    assert_eq!(error_codes(&stderr), ["E0616"], "{stderr}");
}

#[test]
#[doc = "spec: 1.13:2"]
fn a_panic_after_one_valid_replacement_leaves_the_new_valid_value() {
    // Case 11: `set_twice(5, 200)` replaces the value with 5, then panics
    // on 200. The caller catches the panic and reads the `Percent` it
    // lent: it holds 5, which satisfies the invariant, and not 200 and
    // not a half-written value. `set(200)` on the same value refuses and
    // leaves it; `set(7)` replaces it.
    let (directory, rlib) = generated_percent("percent_panic");
    let binary = compile_caller(
        &directory,
        "catches_set_twice",
        "use generated::percent::{Percent, __LocusOption0};\nfn main() {\n    std::panic::set_hook(Box::new(|_| {}));\n    let mut percent = match Percent::checked(3) {\n        __LocusOption0::Some(percent) => percent,\n        __LocusOption0::None => unreachable!(),\n    };\n    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| percent.set_twice(5, 200)));\n    println!(\"panicked {} value {}\", outcome.is_err(), percent.value());\n    assert!(percent.value() <= 100);\n    println!(\"set {} value {}\", percent.set(200), percent.value());\n    println!(\"set {} value {}\", percent.set(7), percent.value());\n}\n",
        &rlib,
    )
    .unwrap_or_else(|stderr| panic!("rustc refused the caller:\n{stderr}"));
    assert_eq!(
        run(binary),
        "panicked true value 5\nset false value 5\nset true value 7\n"
    );
}

#[test]
fn the_impl_block_is_printed_with_its_receivers_and_visibilities() {
    let directory = build_crate_from("percent_layout", "percent.lc", PERCENT);
    let module = std::fs::read_to_string(directory.join("src").join("percent.rs")).unwrap();
    for expected in [
        "\npub struct Percent {",
        "    value: u32,",
        "    in_range: Erased,\n}",
        "pub enum __LocusOption0 {\n    None,\n    Some(Percent),\n}",
        "\nimpl Percent {\n",
        "\n    pub(super) fn new(value: u32, _in_range: Erased) -> Percent {",
        "\n    pub fn checked(value: u32) -> __LocusOption0 {",
        "\n    pub fn value(&self) -> u32 {",
        "\n    pub fn set(&mut self, value: u32) -> bool {",
        "\n    pub fn set_twice(&mut self, a: u32, b: u32) -> () {",
        "\n        *self = Percent { value: a, in_range: Erased };",
        "__LocusOption0::Some(Percent::new(value, Erased))",
    ] {
        assert!(
            module.contains(expected),
            "missing {expected:?} in:\n{module}"
        );
    }
    let root = std::fs::read_to_string(directory.join("src").join("lib.rs")).unwrap();
    assert!(root.contains("pub mod percent;"), "{root}");
}
