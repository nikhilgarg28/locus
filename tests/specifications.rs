use locus::{elab, parser, project, source::SourceMap};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("specifications_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
fn file(dir: &Path, source: &str) -> PathBuf {
    let path = dir.join("export.lc");
    std::fs::write(&path, source).unwrap();
    path
}
fn check_source(name: &str, source: &str) -> Result<(), String> {
    let path = file(&scratch(name), source);
    project::check(&path, &Default::default())
        .map(|_| ())
        .map_err(|e| e.to_string())
}
fn rejected(name: &str, source: &str, expected: &str) {
    let err = check_source(name, source).expect_err(source);
    assert!(err.contains(expected), "{source}\n{err}");
}
fn rustc(dir: &Path, source: &str) -> std::process::Output {
    std::fs::write(dir.join("main.rs"), source).unwrap();
    std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("run"))
        .output()
        .unwrap()
}

#[test]
#[doc = "spec: 1.29:1, 1.29:2, 1.29:6"]
fn external_implementation_and_disjoint_headers_generate_working_rust() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/specs/split");
    let checked = project::check(&fixture, &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let rust = project::rust(checked).unwrap();
    let dir = scratch("split_rust");
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let result = rustc(
        &dir,
        "include!(\"generated.rs\"); fn main() { assert_eq!(answer(),42); assert_eq!(arithmetic::next(255),0); assert_eq!(arithmetic::BASE,41); }",
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
    let hostile = rustc(
        &dir,
        "include!(\"generated.rs\"); fn main(){let _=arithmetic::helper(0);}",
    );
    assert!(!hostile.status.success());
    let loaded = project::load(&fixture).unwrap();
    assert_eq!(loaded.inputs.len(), 2);
}

#[test]
#[doc = "spec: 1.29:3"]
fn missing_duplicate_and_unlisted_members_fail_even_when_unused() {
    for (i, source) in [
        "spec mod unused { fn f()->u8; }",
        "spec mod unused { fn f()->u8; } impl mod unused {}",
        "spec mod m { fn f()->u8; fn f()->u8; } impl mod m {fn f()->u8{0}}",
        "spec mod m { fn f()->u8; } spec mod m { fn f()->u8; } impl mod m {fn f()->u8{0}}",
        "spec mod m {} impl mod m {} impl mod m {}",
        "impl mod absent {}",
        "spec mod m {} impl mod m {pub fn extra()->u8{0}}",
        "spec mod m {} impl mod m {pub(crate) fn extra()->u8{0}}",
        "spec mod m {fn f()->u8;} impl mod m {pub(crate) fn f()->u8{0}}",
        "pub spec mod m {} spec mod m {} impl mod m {}",
        "spec type T { fn new()->T; }",
    ]
    .iter()
    .enumerate()
    {
        rejected(&format!("completion{i}"), source, "L0511");
    }
}

#[test]
#[doc = "spec: 1.29:4"]
fn signature_comparison_preserves_proofs_binders_and_logic_mode() {
    for (i, (header, body)) in [
        ("fn f(x:u8)->u8;", "fn f(x:u16)->u8 {0}"),
        ("fn f(x:u8)->u8;", "fn f(y:u8)->u8 {0}"),
        ("fn f(x:u8,p:@(x==0))->u8;", "fn f(x:u8)->u8 {x}"),
        ("fn f()->@(1==0);", "fn f()->@(1==1) {_}"),
        ("logic fn f(x:Int)->Int;", "fn f(x:Int)->Int{x}"),
        ("const VALUE:u8;", "const VALUE:u16=0;"),
        ("const VALUE:u8;", "fn VALUE()->u8{0}"),
    ]
    .iter()
    .enumerate()
    {
        rejected(
            &format!("signature{i}"),
            &format!("spec mod m {{{header}}} impl mod m {{{body}}}"),
            "L0511",
        );
    }
    check_source(
        "whitespace",
        "spec mod m { fn f(x: u8) -> u8; } impl mod m { fn f( /* comment */ x:u8)->u8{x} }",
    )
    .unwrap();
}

#[test]
#[doc = "spec: 1.29:4, 1.29:5"]
fn promised_facts_and_effects_still_require_checked_implementations() {
    rejected(
        "false_proof",
        "spec mod m {fn bad()->@(1==0);} impl mod m {fn bad()->@(1==0){_}}",
        "L0230",
    );
    rejected(
        "panic_promise",
        "spec mod m {#[no_panic] fn inc(n:u8)->u8;} impl mod m {fn inc(n:u8)->u8{n+1}}",
        "L0235",
    );
    rejected(
        "pure_promise",
        "spec mod m {logic fn bad(n:Int)->Int;} impl mod m {logic fn bad(n:Int)->Int{loop {}}}",
        "error",
    );
    rejected(
        "recursive_proof",
        "spec mod m {fn bad()->@(1==0);} impl mod m {fn bad()->@(1==0){bad()}}",
        "error",
    );
    check_source("checked_logic", "spec mod m {logic fn same(n:Int)->@(n==n);} impl mod m {logic fn same(n:Int)->@(n==n){_}} fn main()->@(3==3){m::same(3)}").unwrap();
    rejected(
        "missing_call_proof",
        "spec mod m {fn f(n:u8,p:@(n==0))->u8;} impl mod m {fn f(n:u8,p:@(n==0))->u8{n}} fn main()->u8{m::f(1,_)}",
        "L0230",
    );
}

#[test]
#[doc = "spec: 1.29:2, 1.29:5"]
fn opaque_type_representation_carries_checked_invariant_fields() {
    let source = r#"
mod counter {
    pub spec type Counter { fn new()->Counter; fn get(&self)->u8; }
    struct Counter { value:u8, valid:@(value==1) }
    impl Counter {
        fn new()->Counter {Counter {value:1, valid:_}}
        fn get(&self)->u8 {self.value}
        fn hidden(&self)->u8 {self.value}
    }
}
pub use counter::Counter;
"#;
    let dir = scratch("type_rust");
    let checked =
        project::check(&file(&dir, source), &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let rust = project::rust(checked).unwrap();
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let result = rustc(
        &dir,
        "include!(\"generated.rs\"); fn main(){assert_eq!(Counter::new().get(),1);}",
    );
    assert!(
        result.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&result.stderr)
    );
    for body in [
        "let _=Counter::new().hidden();",
        "let mut x=Counter::new();x.value=2;",
        "let _=Counter {value:0};",
    ] {
        assert!(
            !rustc(
                &dir,
                &format!("include!(\"generated.rs\");fn main(){{{body}}}")
            )
            .status
            .success()
        );
    }
    rejected(
        "bad_struct_proof",
        &source.replace("value:1, valid:_", "value:2, valid:_"),
        "L0230",
    );
    rejected(
        "private_field",
        &format!("{source} fn bad()->u8{{counter::Counter::new().value}}"),
        "L0503",
    );
}

#[test]
#[doc = "spec: 1.29:3"]
fn aliases_and_other_modules_cannot_extend_a_spec_type() {
    let base = "mod m {pub spec type T {} struct T {n:u8} impl T {}}";
    for (i, suffix) in [
        "use m::T as Alias; impl Alias {pub fn added()->u8{0}}",
        "impl m::T {pub fn added()->u8{0}}",
        "mod n {impl crate::m::T {fn hidden()->u8{0}}}",
    ]
    .iter()
    .enumerate()
    {
        rejected(&format!("alias{i}"), &format!("{base} {suffix}"), "L0511");
    }
}

#[test]
#[doc = "spec: 1.29:1"]
fn unsupported_header_forms_are_rejected_deliberately() {
    for (i, s) in [
        "spec mod m {pub fn f()->u8;}",
        "spec mod m<T> {}",
        "spec type T {fn f<X>(x:X)->X;}",
        "spec mod m {struct T {n:u8}}",
        "spec mod m {use crate::x;}",
        "spec type T {} struct T {pub n:u8} impl T {}",
        "spec type T {} #[derive(Copy)] struct T {n:u8} impl T {}",
    ]
    .iter()
    .enumerate()
    {
        rejected(&format!("unsupported{i}"), s, "L0510");
    }
}

#[test]
fn original_files_are_shown_for_signature_mismatch() {
    let dir = scratch("spans");
    std::fs::write(dir.join("m.lc"), "fn f(x:u16)->u8{0}").unwrap();
    let path = file(&dir, "spec mod m {fn f(x:u8)->u8;} impl mod m;");
    let error = project::load(&path).err().unwrap().to_string();
    assert!(error.contains("export.lc:1"), "{error}");
    assert!(error.contains("m.lc:1"), "{error}");
}

#[test]
fn flat_elaboration_checks_specs_instead_of_ignoring_headers() {
    let mut sources = SourceMap::default();
    let id = sources.add(
        "inline.lc",
        "spec mod m {fn f()->u8;} impl mod m {fn f()->u8{7}} fn main()->u8{m::f()}",
    );
    let source = sources.get(id);
    let parsed = parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let checked = elab::elaborate(source, &parsed.program);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert!(checked.function("main").is_some());
}

#[test]
#[doc = "spec: 1.29:5, 1.29:6"]
fn proof_only_mutation_runs_and_rust_cannot_supply_proof_inputs() {
    let source = r#"
pub spec mod state {
    fn clear(value: &mut u8) -> @(value == 0);
}
impl mod state {
    fn clear(value: &mut u8) -> @(value == 0) { value = 0; _ }
}
pub fn run() -> u8 {
    let mut value:u8=7;
    let cleared=state::clear(&mut value);
    value
}
"#;
    let dir = scratch("mutation");
    let checked =
        project::check(&file(&dir, source), &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let rust = project::rust(checked).unwrap();
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let result = rustc(
        &dir,
        "include!(\"generated.rs\");fn main(){let mut value=9;state::clear(&mut value);assert_eq!(value,0);assert_eq!(run(),0);}",
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
    rejected(
        "wrong_final_snapshot",
        &source.replace("value = 0; _", "value = 1; _"),
        "L0230",
    );
    let taking =
        "pub spec mod m {fn f(n:u8,p:@(n==0))->u8;} impl mod m {fn f(n:u8,p:@(n==0))->u8{n}}";
    let checked =
        project::check(&file(&dir, taking), &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let error = project::rust(checked).err().unwrap().to_string();
    assert!(error.contains("L0504"), "{error}");
}

#[test]
fn an_intervening_sibling_does_not_gain_implementation_privileges() {
    let header = "spec mod m {fn make()->Hidden;}";
    let body = "impl mod m {struct Hidden {n:u8} fn make()->Hidden{Hidden {n:1}}}";
    for (i, source) in [
        format!("{header} fn attack()->u8{{m::make().n}} {body}"),
        format!("{body} fn attack()->u8{{m::make().n}} {header}"),
    ]
    .iter()
    .enumerate()
    {
        rejected(&format!("sibling{i}"), source, "L0503");
    }
}

#[test]
fn type_constants_receivers_and_private_helpers_follow_the_contract() {
    check_source(
        "type_constants",
        r#"
spec type Counter { const START:u8; fn new()->Counter; fn take(self)->u8; }
struct Counter {n:u8}
impl Counter {
    const START:u8=7;
    fn new()->Counter { Counter {n:Counter::START} }
    fn take(self)->u8 {self.n}
    fn helper()->u8 {9}
}
fn main()->u8 { Counter::new().take() }
"#,
    )
    .unwrap();
    rejected(
        "receiver_mismatch",
        "spec type T {fn read(&self)->u8;} struct T {n:u8} impl T {fn read(self)->u8{self.n}}",
        "L0511",
    );
    check_source(
        "contextual_spec",
        "fn spec(x:u8)->u8{x} fn main()->u8{spec(1)}",
    )
    .unwrap();
}

#[test]
fn logical_definitions_are_checked_and_remain_transparent() {
    check_source(
        "logical_definition",
        r#"
spec mod math {logic fn identity(n:Int)->Int;}
impl mod math {logic fn identity(n:Int)->Int{n}}
fn theorem(n:Int)->@(math::identity(n)==n) {
    fold!(math::identity, prove!(n==n))
}
"#,
    )
    .unwrap();
    let mut sources = SourceMap::default();
    let id = sources.add(
        "missing.lc",
        "spec mod missing {logic fn falsehood()->@(1==0);}",
    );
    let source = sources.get(id);
    let parsed = parser::parse(source);
    assert!(parsed.is_success());
    let checked = elab::elaborate(source, &parsed.program);
    assert!(!checked.is_success());
    assert!(checked.functions.is_empty());
    assert!(checked.diagnostics.iter().any(|d| d.code == "L0511"));
}

#[test]
fn ordinary_trusted_headers_do_not_satisfy_manual_specs() {
    rejected(
        "trusted_body",
        r#"
spec mod m {fn f(n:u8)->u8;}
impl mod m {trusted "native" fn f(n:u8)->u8 = Other::f;}
"#,
        "L0511",
    );
}

#[test]
fn file_loading_failures_and_type_spec_cli_entry_are_explicit() {
    let dir = scratch("discovery");
    let path = file(&dir, "spec mod absent {} impl mod absent;");
    assert!(
        project::load(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("L0501")
    );
    std::fs::write(dir.join("absent.lc"), "").unwrap();
    std::fs::create_dir(dir.join("absent")).unwrap();
    std::fs::write(dir.join("absent/mod.lc"), "").unwrap();
    assert!(
        project::load(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("ambiguous")
    );
    let entry = dir.join("component.lc");
    std::fs::write(
        &entry,
        "pub spec type T {fn new()->T;} struct T {n:u8} impl T {fn new()->T{T {n:1}}}",
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_locus"))
        .args(["rust"])
        .arg(&entry)
        .arg("--no-store")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("__exports"));
}

#[test]
fn header_type_nesting_is_bounded_without_losing_parser_progress() {
    let mut sources = SourceMap::default();
    let deep = format!(
        "spec mod m {{fn f(x:{}u8{})->u8;}}",
        "(".repeat(500),
        ")".repeat(500)
    );
    let id = sources.add("deep.lc", deep);
    let parsed = parser::parse(sources.get(id));
    assert!(!parsed.is_success());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("nesting")),
        "{:?}",
        parsed.diagnostics
    );
    assert!(parsed.stats.steps <= 4 * parsed.stats.tokens);
}
