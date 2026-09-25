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
fn external_implementation_and_headers_generate_working_rust() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/specs/split");
    let checked = project::check(&fixture, &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let rust = project::rust(checked).unwrap();
    let dir = scratch("split_rust");
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let out = rustc(
        &dir,
        "include!(\"generated.rs\"); fn main(){ assert_eq!(answer(),42); assert_eq!(Arithmetic::next(255),0); assert_eq!(Arithmetic::BASE,41); }",
    );
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("run"))
            .status()
            .unwrap()
            .success()
    );
    assert!(
        !rustc(
            &dir,
            "include!(\"generated.rs\");fn main(){Arithmetic::helper(0);}"
        )
        .status
        .success()
    );
    assert_eq!(project::load(&fixture).unwrap().inputs.len(), 2);
}
#[test]
#[doc = "spec: 1.29:3"]
fn complete_unique_implementation_required_even_when_unused() {
    for (i, code) in [
        "spec type S{}",
        "spec type S{fn f()->u8;} struct R{} impl S for R{}",
        "spec type S{} struct R{} impl S for R{} impl S for R{}",
        "spec type S{fn f()->u8;fn f()->u8;} struct R{} impl S for R{fn f()->u8{0}}",
        "spec type S{} struct R{} impl S for R{fn extra()->u8{0}}",
        "spec type S{fn f()->u8;} struct R{} impl S for R{pub fn f()->u8{0}}",
        "struct R{} impl Missing for R{}",
        "spec type S{} impl S for S{}",
    ]
    .iter()
    .enumerate()
    {
        rejected(&format!("complete{i}"), code, "L0511");
    }
}
#[test]
#[doc = "spec: 1.29:4"]
fn resolved_signatures_preserve_proofs_and_modes() {
    for (i, (head, body)) in [
        ("fn f(x:u8)->u8;", "fn f(x:u16)->u8{0}"),
        ("fn f(x:u8,p:@(x==0))->u8;", "fn f(x:u8)->u8{x}"),
        ("fn f()->@(1==0);", "fn f()->@(1==1){_}"),
        ("logic fn f(x:Int)->Int;", "fn f(x:Int)->Int{x}"),
        ("const VALUE:u8;", "const VALUE:u16=0;"),
        ("const VALUE:u8;", "fn VALUE()->u8{0}"),
        ("fn f(&self)->u8;", "fn f(self)->u8{0}"),
    ]
    .iter()
    .enumerate()
    {
        rejected(
            &format!("signature{i}"),
            &format!("spec type S{{{head}}} struct R{{}} impl S for R{{{body}}}"),
            "L0511",
        );
    }
    check_source("alpha", "spec type S{fn f(x:u8,p:@(x==0))->(out:u8,@(out==x));} struct R{} impl S for R{fn f(y:(u8),q:@((y==0)))->(result:u8,@(result==y)){(y,_)}}").unwrap();
}
#[test]
#[doc = "spec: 1.29:4, 1.29:5"]
fn evidence_and_effect_promises_are_checked() {
    for (tag, head, body, code) in [
        ("false", "fn f()->@(1==0);", "fn f()->@(1==0){_}", "L0230"),
        (
            "panic",
            "#[no_panic] fn f(x:u8)->u8;",
            "fn f(x:u8)->u8{x+1}",
            "L0235",
        ),
        (
            "logic",
            "logic fn f(x:Int)->Int;",
            "logic fn f(x:Int)->Int{loop{}}",
            "error",
        ),
        (
            "recursion",
            "fn f()->@(1==0);",
            "fn f()->@(1==0){Self::f()}",
            "error",
        ),
        (
            "native",
            "fn f(x:u8)->u8;",
            "trusted \"native\" fn f(x:u8)->u8=Other::f;",
            "L0511",
        ),
    ] {
        rejected(
            tag,
            &format!("spec type S{{{head}}}struct R{{}}impl S for R{{{body}}}"),
            code,
        );
    }
    rejected(
        "missing_input",
        "spec type S{fn f(n:u8,p:@(n==0))->u8;}struct R{}impl S for R{fn f(n:u8,p:@(n==0))->u8{n}}fn main()->u8{S::f(1,_)}",
        "L0230",
    );
}
#[test]
#[doc = "spec: 1.29:2, 1.29:5"]
fn opaque_representation_keeps_its_invariant() {
    let source = r#"
mod m {
 pub spec type Percent {fn zero()->Self;fn get(&self)->u8;}
 struct Repr {n:u8,valid:@(n<=100)}
 impl Repr {fn hidden(&self)->u8{self.n}}
 impl Percent for Repr {fn zero()->Self{Self{n:0,valid:_}}fn get(&self)->u8{self.hidden()}}
}
pub use m::Percent;
"#;
    let dir = scratch("invariant");
    let checked =
        project::check(&file(&dir, source), &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let rust = project::rust(checked).unwrap();
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let out = rustc(
        &dir,
        "include!(\"generated.rs\");fn main(){assert_eq!(Percent::zero().get(),0);}",
    );
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    for code in [
        "let _=Percent::zero().hidden();",
        "let mut p=Percent::zero();p.n=255;",
        "let _=Percent{n:0};",
        "let _=Percent::zero().__locus_repr;",
    ] {
        assert!(
            !rustc(
                &dir,
                &format!("include!(\"generated.rs\");fn main(){{{code}}}")
            )
            .status
            .success()
        );
    }
    rejected(
        "false_invariant",
        &source.replace("n:0,valid:_", "n:255,valid:_"),
        "L0230",
    );
    rejected(
        "hidden_field",
        &format!("{source}fn bad()->u8{{let p=m::Percent::zero();p.n}}"),
        "error",
    );
    rejected(
        "hidden_repr",
        &format!("{source}fn bad()->(){{let p=m::Percent::zero();p.__locus_repr;}}"),
        "error",
    );
}
#[test]
#[doc = "spec: 1.29:3"]
fn aliases_do_not_add_methods_or_conversions() {
    let base = "mod m{pub spec type S{} pub struct R{pub n:u8} impl S for R{}}";
    for (i, suffix) in [
        "use m::S as Alias;impl Alias{fn extra()->u8{0}}",
        "impl m::S{}",
        "fn cast(r:m::R)->m::S{r}",
    ]
    .iter()
    .enumerate()
    {
        rejected(&format!("alias{i}"), &format!("{base}{suffix}"), "error");
    }
}
#[test]
#[doc = "spec: 1.29:1"]
fn deferred_grammar_is_rejected_deliberately() {
    for (i,code) in ["spec mod m{}","impl mod m{}","spec type S{pub fn f()->u8;}","spec type S{fn f<T>(x:T)->T;}struct R{}impl S for R{fn f<T>(x:T)->T{x}}","spec type S{type A;type B;}struct R{}impl S for R{type A=Self::B;type B=Self::A;}","spec type S{fn identity(x:&Self)->&Self;}struct R{}impl S for R{fn identity(x:&Self)->&Self{x}}"].iter().enumerate(){rejected(&format!("deferred{i}"),code,"L0510");}
}
#[test]
fn diagnostic_keeps_both_source_locations() {
    let dir = scratch("spans");
    std::fs::write(
        dir.join("body.lc"),
        "struct R{} impl crate::S for R{fn f(x:u16)->u8{0}}",
    )
    .unwrap();
    let err = project::load(&file(&dir, "spec type S{fn f(x:u8)->u8;}mod body;"))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("export.lc:1"), "{err}");
    assert!(err.contains("body.lc:1"), "{err}");
}
#[test]
fn flat_elaboration_checks_specs() {
    let mut sources = SourceMap::default();
    let id = sources.add(
        "inline.lc",
        "spec type S{fn f()->u8;}struct R{}impl S for R{fn f()->u8{7}}fn main()->u8{S::f()}",
    );
    let source = sources.get(id);
    let parsed = parser::parse(source);
    assert!(parsed.is_success());
    let checked = elab::elaborate(source, &parsed.program);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
}
#[test]
#[doc = "spec: 1.29:5, 1.29:6"]
fn proof_only_mutation_runs_and_proof_inputs_cannot_export() {
    let code = "pub spec type State{fn clear(value:&mut u8)->@(value==0);}struct R{}impl State for R{fn clear(value:&mut u8)->@(value==0){value=0;_}}pub fn run()->u8{let mut value:u8=7;let p=State::clear(&mut value);value}";
    let dir = scratch("mutation");
    let checked =
        project::check(&file(&dir, code), &Default::default()).unwrap_or_else(|e| panic!("{e}"));
    let rust = project::rust(checked).unwrap();
    std::fs::write(dir.join("generated.rs"), &rust).unwrap();
    let out = rustc(
        &dir,
        "include!(\"generated.rs\");fn main(){let mut x=9;State::clear(&mut x);assert_eq!(x,0);assert_eq!(run(),0);}",
    );
    assert!(
        out.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("run"))
            .status()
            .unwrap()
            .success()
    );
    rejected(
        "final_snapshot",
        &code.replace("value=0;_", "value=1;_"),
        "L0230",
    );
    let checked=project::check(&file(&dir,"pub spec type S{fn f(n:u8,p:@(n==0))->u8;}struct R{}impl S for R{fn f(n:u8,p:@(n==0))->u8{n}}"),&Default::default()).unwrap_or_else(|e|panic!("{e}"));
    assert!(
        project::rust(checked)
            .unwrap_err()
            .to_string()
            .contains("L0504")
    );
}
#[test]
fn logical_definitions_remain_transparent() {
    check_source("logical","spec type Math{logic fn same(n:Int)->@(n==n);}struct R{}impl Math for R{logic fn same(n:Int)->@(n==n){_}}fn main()->@(3==3){Math::same(3)}").unwrap();
    rejected(
        "unused_logical",
        "spec type Missing{logic fn falsehood()->@(1==0);}",
        "L0511",
    );
}
#[test]
fn parser_nesting_limit_covers_headers() {
    let mut sources = SourceMap::default();
    let id = sources.add(
        "deep.lc",
        format!(
            "spec type S{{fn f(x:{}u8{})->u8;}}",
            "(".repeat(500),
            ")".repeat(500)
        ),
    );
    let parsed = parser::parse(sources.get(id));
    assert!(!parsed.is_success());
    assert!(
        parsed
            .diagnostics
            .iter()
            .any(|d| d.message.contains("nesting"))
    );
    assert!(parsed.stats.steps <= 4 * parsed.stats.tokens);
}
