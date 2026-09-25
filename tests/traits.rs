use locus::project;
use std::path::PathBuf;
fn check(tag: &str, code: &str) -> Result<project::Checked, String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("trait_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.lc");
    std::fs::write(&path, code).unwrap();
    project::check(&path, &Default::default()).map_err(|e| e.to_string())
}
#[test]
#[doc = "spec: 1.31:1"]
#[doc = "spec: 1.31:5"]
fn methods_and_defaults() {
    check(
        "methods",
        r#"
trait Read { fn read(&self)->u8; fn twice(&self)->u8 {self.read()+self.read()} }
struct Counter {n:u8}
impl Read for Counter {fn read(&self)->u8{self.n}}
fn main()->u8 {let c=Counter{n:3};c.twice()}
"#,
    )
    .unwrap();
}
#[test]
#[doc = "spec: 1.31:2"]
fn qualified_and_associated() {
    check("qualified",r#"
trait Read {type Item; const ZERO:Self::Item;fn read(&self)->Self::Item;}
struct Counter {n:u8}
impl Read for Counter {type Item=u8; const ZERO:u8=0;fn read(&self)->u8{self.n}}
fn main()-><Counter as Read>::Item {let c=Counter{n:<Counter as Read>::ZERO};<Counter as Read>::read(&c)}
"#).unwrap();
}
#[test]
#[doc = "spec: 1.31:3"]
fn proof_contract_and_renamed_binders() {
    check("proof",r#"
trait Step { fn next(n:u8)->(out:u8,@(out==n.wrapping_add(1))); }
struct Counter {}
impl Step for Counter {fn next(value:u8)->(result:u8,@(result==value.wrapping_add(1))) {let result=value.wrapping_add(1);(result,_)}}
fn main()->u8{let (out,p)=<Counter as Step>::next(2);out}
"#).unwrap();
}
#[test]
#[doc = "spec: 1.31:7"]
fn missing_mismatched_and_conflicting() {
    for (i, code) in [
        "trait A{fn f()->u8;}struct S{}impl A for S{}",
        "trait A{fn f()->u8;}struct S{}impl A for S{fn f()->bool{true}}",
        "trait A{}struct S{}impl A for S{}impl A for S{}",
        "trait A{}struct S{}impl A for S{fn extra()->(){}}",
        "trait A{fn f()->@(1==0);}struct S{}impl A for S{fn f()->@(1==0){_}}",
        "trait A{}impl A for u8{}",
        "trait A{fn f(&self)->u8;}struct S{}impl A for S{fn f(self)->u8{1}}",
    ]
    .iter()
    .enumerate()
    {
        let error = check(&format!("negative{i}"), code).err().unwrap();
        assert!(
            error.contains("L0515") || error.contains("L0230"),
            "{error}"
        );
    }
}
#[test]
fn rust_export_preserves_trait_identity_and_methods() {
    let checked = check(
        "rust",
        r#"
pub trait Read { type Item; const ZERO: Self::Item; fn read(&self)->Self::Item; fn twice(&self)->u8 { 2 } }
pub struct Counter {pub n:u8}
impl Read for Counter {type Item=u8;const ZERO:u8=0;fn read(&self)->u8{self.n}}
pub fn answer()->u8 {let c=Counter{n:42};c.read()}
"#,
    )
    .unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("trait_rust");
    std::fs::write(dir.join("main.rs"),format!("{rust}\nfn main() {{ use Read; let c=Counter{{n:42}}; assert_eq!(c.read(),42); assert_eq!(c.twice(),2); assert_eq!(Counter::ZERO,0); assert_eq!(answer(),42); }}")).unwrap();
    let result = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("app"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("app"))
            .status()
            .unwrap()
            .success()
    );
}
#[test]
#[doc = "spec: 1.31:9"]
fn export_logical_members_and_promises_fails() {
    for (i, code) in [
        "pub trait A {fn f()->@(1==1);}",
        "pub trait A {logic fn f()->Nat;}",
        "pub trait A {#[no_panic] fn f()->u8;}",
        "pub trait A {type View:Logical;}",
        "pub trait A {type Item;}struct S{}impl A for S{type Item=Nat;}",
    ]
    .iter()
    .enumerate()
    {
        let error = project::rust(check(&format!("export_bad{i}"), code).unwrap())
            .err()
            .unwrap();
        assert!(error.to_string().contains("L0504"), "{error}");
    }
}
#[test]
#[doc = "spec: 1.31:4"]
fn logical_associated_and_methods() {
    check(
        "logical_assoc",
        r#"
trait View { type Output: Logical; logic fn identity(x:Self::Output)->Self::Output; }
struct S{}
impl View for S {type Output=Nat;logic fn identity(x:Nat)->Nat{x}}
fn identity(n:Nat)->@(S::identity(n)==n){fold!(S::identity,prove!(n==n))}
"#,
    )
    .unwrap();
    let error = check(
        "not_logical_assoc",
        "trait A{type X:Logical;}struct S{}impl A for S{type X=u8;}",
    )
    .err()
    .unwrap();
    assert!(error.contains("logical"), "{error}");
}
#[test]
fn defaults_are_rechecked_after_override_and_cycles_rejected() {
    let source = r#"
trait Law {
 logic fn value(n:Int)->Int{n}
 fn law(n:Int)->@(Self::value(n)==n){fold!(Self::value,prove!(n==n))}
}
struct Honest{}
impl Law for Honest{}
"#;
    check("honest", source).unwrap();
    let bad =
        format!("{source}struct Wrong{{}}impl Law for Wrong{{logic fn value(n:Int)->Int{{n+1}}}}");
    let error = check("override_false", &bad).err().unwrap();
    assert!(error.contains("L0230") || error.contains("L027"), "{error}");
    let error = check(
        "cycle",
        "trait A{logic fn a()->Int{Self::b()}logic fn b()->Int{Self::a()}}struct S{}impl A for S{}",
    )
    .err()
    .unwrap();
    assert!(error.contains("L0203"), "{error}");
}
#[test]
#[doc = "spec: 1.31:6"]
fn scope_ambiguity_and_inherent_precedence() {
    let source = r#"
mod a{pub trait A {fn f(&self)->u8;}}
mod b{pub trait B {fn f(&self)->u8;}}
struct S{}
impl a::A for S {fn f(&self)->u8{1}}
impl b::B for S {fn f(&self)->u8{2}}
"#;
    check(
        "scope_ok",
        &format!("{source}use a::A;fn run()->u8{{let s=S{{}};s.f()}}"),
    )
    .unwrap();
    let error = check(
        "scope_none",
        &format!("{source}fn run()->u8{{let s=S{{}};s.f()}}"),
    )
    .err()
    .unwrap();
    assert!(error.contains("L0207"), "{error}");
    let error = check(
        "scope_ambiguous",
        &format!("{source}use a::A;use b::B;fn run()->u8{{let s=S{{}};s.f()}}"),
    )
    .err()
    .unwrap();
    assert!(error.contains("L0516"), "{error}");
    check(
        "scope_qualified",
        &format!("{source}fn run()->u8{{let s=S{{}};<S as a::A>::f(&s)}}"),
    )
    .unwrap();
    check("inherent_first",&format!("{source}use a::A;use b::B;impl S{{fn f(&self)->u8{{3}}}}fn run()->u8{{let s=S{{}};s.f()}}" )).unwrap();
}
fn run(tag: &str, code: &str, expected: u8) {
    let checked = check(tag, code).unwrap();
    let function = checked
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("_answer"))
        .unwrap()
        .1;
    let a = locus::exec::CheckInterpreter::new(checked.checked.session.program(), 10000)
        .with_lending(checked.checked.session.lending())
        .call(function, vec![])
        .unwrap();
    let b = locus::erased::Interpreter::new(checked.checked.session.erased(), 10000)
        .call(function, vec![])
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(format!("{a:?}"), format!("Value(Int(U8, {expected}))"));
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("trait_{tag}_rust"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{assert_eq!(answer(),{expected});}}"),
    )
    .unwrap();
    for checks in ["yes", "no"] {
        let result = std::process::Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "-C"])
            .arg(format!("overflow-checks={checks}"))
            .arg(dir.join("main.rs"))
            .arg("-o")
            .arg(dir.join("app"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            std::process::Command::new(dir.join("app"))
                .status()
                .unwrap()
                .success()
        );
    }
}
#[test]
fn concrete_dispatch_interpreters_and_associated_calls() {
    run(
        "dispatch",
        r#"
trait Amount {fn get(&self)->u8;fn base()->u8{1}}
struct A{} struct B{}
impl Amount for A{fn get(&self)->u8{3}}
impl Amount for B{fn get(&self)->u8{7}}
pub fn answer()->u8 {let a=A{};let b=B{};a.get()+b.get()+A::base()}
"#,
        11,
    );
}
#[test]
#[doc = "spec: 1.31:8"]
fn proof_only_runtime_method_preserves_mutation() {
    run(
        "mutation",
        r#"
trait Change { fn bump(n:&mut u8)->@(n==old!(n).wrapping_add(1)); }
struct S{}
impl Change for S {fn bump(n:&mut u8)->@(n==old!(n).wrapping_add(1)){n=n.wrapping_add(1);_}}
pub fn answer()->u8{let mut n:u8=7;let p=S::bump(&mut n);n}
"#,
        8,
    );
}
#[test]
fn proof_inputs_and_contract_substitution() {
    check(
        "input",
        r#"
trait Step {#[no_panic]fn next(n:u8,p:@(n<255))->(out:u8,@(out==n+1));}
struct S{}
impl Step for S {fn next(x:u8,h:@(x<255))->(y:u8,@(y==x+1)){let y=x+1;(y,_)}}
fn answer()->u8 {let (x,p)=S::next(2,prove!(2<255));x}
"#,
    )
    .unwrap();
    for (i, body) in [
        "fn next(x:u8,h:@(x<255))->(y:u8,@(y==x+2)){let y=x+1;(y,_)}",
        "fn next(x:u8,h:@(x<254))->(y:u8,@(y==x+1)){let y=x+1;(y,_)}",
        "logic fn next(x:u8,h:@(x<255))->(y:u8,@(y==x+1)){let y=x+1;(y,_)}",
    ]
    .iter()
    .enumerate()
    {
        let error=check(&format!("contract{i}"),&format!("trait Step{{fn next(n:u8,p:@(n<255))->(out:u8,@(out==n+1));}}struct S{{}}impl Step for S{{{body}}}")).err().unwrap();
        assert!(error.contains("L0515"), "{error}");
    }
}
#[test]
fn unsupported_shapes_and_trait_type_misuse() {
    for (i, code) in [
        "trait A<T>{}",
        "trait A:B{}",
        "trait A{fn f<T>(x:T)->T;}",
        "trait A{}struct S<T>{x:T}impl<T>A for S<T>{}",
        "trait A{}fn f(x:A)->A{x}",
        "trait A{}struct Bad{x:A}",
        "trait A{type X;}struct S{}impl A for S{type X=Self::X;}",
        "trait A{fn f()->();}struct S{}impl A for S{pub fn f()->(){}}",
        "trait Logical{}",
        "trait Model{}",
        "trait Copy{}",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            check(&format!("unsupported{i}"), code).is_err(),
            "accepted {code}"
        );
    }
}
#[test]
fn logical_mode_and_promises_are_body_obligations() {
    for (i,code) in [
        "trait A{logic fn f()->Int;}struct S{}impl A for S{logic fn f()->Int{loop{}}}",
        "trait A{#[no_panic]fn f(x:u8)->u8;}struct S{}impl A for S{fn f(x:u8)->u8{x+1}}",
        "trait A{logic fn f(n:&mut u8)->Prop;}struct S{}impl A for S{logic fn f(n:&mut u8)->Prop{prop!(true)}}",
    ].iter().enumerate(){assert!(check(&format!("effects{i}"),code).is_err(),"accepted {code}");}
    check("unused_trait", "trait A{fn unimplemented()->@(false);}").unwrap();
}
#[test]
fn spec_identity_does_not_inherit_backing_traits() {
    let source = r#"
trait Read {fn read(&self)->u8;}
spec type Opaque{fn new()->Self;}
struct Repr{n:u8}
impl Opaque for Repr{fn new()->Self{Self{n:7}}}
impl Read for Repr{fn read(&self)->u8{self.n}}
"#;
    let error = check(
        "spec_hidden",
        &format!("{source}fn answer()->u8{{let x=Opaque::new();x.read()}}"),
    )
    .err()
    .unwrap();
    assert!(error.contains("L0207"), "{error}");
    check("spec_forward",&format!("{source}impl Read for Opaque{{fn read(&self)->u8{{7}}}}fn answer()->u8{{let x=Opaque::new();x.read()}}" )).unwrap();
}
#[test]
fn split_files_and_cli_use_the_checked_export_boundary() {
    let fixture =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/traits/split/export.lc");
    let checked = project::check(&fixture, &Default::default()).unwrap();
    let rust = project::rust(checked).unwrap();
    assert!(rust.contains("answer"));
    let file = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("trait_cli.lc");
    std::fs::write(&file, "pub trait Bad{fn claim()->@(false);}").unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_locus"))
        .arg("rust")
        .arg(&file)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("L0504"));
}

#[test]
#[doc = "spec: 1.31:3"]
fn qualified_contracts_normalize_before_signature_comparison() {
    check(
        "qualified_contract",
        r#"
trait A {
 type Item;
 logic fn id(n:Int)->Int;
 fn law(n:Int)->@(Self::id(n)==n);
 fn take(n:Self::Item)->Self::Item;
}
struct S{}
impl A for S {
 type Item=u8;
 logic fn id(n:Int)->Int{n}
 fn law(n:Int)->@(<Self as A>::id(n)==n) {
   fold!(<Self as A>::id,prove!(n==n))
 }
 fn take(n:<Self as A>::Item)-><S as A>::Item{n}
}
fn use_law(n:Int)->@(<S as A>::id(n)==n){S::law(n)}
"#,
    )
    .unwrap();
}

#[test]
fn distinct_case_sensitive_trait_identities_do_not_collide() {
    run(
        "case_identity",
        r#"
trait Aa { fn f()->u8; }
trait AA { fn f()->u8; }
struct S{}
impl Aa for S{fn f()->u8{1}}
impl AA for S{fn f()->u8{2}}
pub fn answer()->u8{<S as Aa>::f()+<S as AA>::f()}
"#,
        3,
    );
}

#[test]
#[doc = "spec: 1.31:8"]
fn selected_mutation_invalidates_tracked_evidence() {
    let prefix = r#"
trait Mutate {
 fn change(n:&mut u8)->@(n<=3);
 fn take(n:u8,p:@(n<=3))->u8;
}
struct S{}
impl Mutate for S {
 fn change(n:&mut u8)->@(n<=3){n=3;_}
 fn take(n:u8,p:@(n<=3))->u8{n}
}
"#;
    let body = r#"
fn example()->u8{
 let mut n:u8=2;
 let mut p:@(n<=3)=prove!(n<=3);
 let established=S::change(&mut n);
 S::take(n,p)
}
"#;
    let error = check("stale_trait", &format!("{prefix}{body}"))
        .err()
        .unwrap();
    assert!(error.contains("L0245"), "{error}");
    let refreshed = body.replace("S::take(n,p)", "p=established;S::take(n,p)");
    check("refreshed_trait", &format!("{prefix}{refreshed}")).unwrap();
}
