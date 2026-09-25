use locus::project;
use std::path::PathBuf;
fn check(tag: &str, code: &str) -> Result<project::Checked, String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("spec_revision_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.lc");
    std::fs::write(&path, code).unwrap();
    project::check(&path, &Default::default()).map_err(|e| e.to_string())
}
#[test]
fn basic() {
    check(
        "basic",
        r#"
spec type Counter { fn new()->Self; fn get(&self)->u8; fn bump(&mut self)->(); }
struct Repr {n:u8}
impl Counter for Repr {
 fn new()->Self{Self{n:0}}
 fn get(&self)->u8{self.n}
 fn bump(&mut self)->(){self.n=self.n+1;}
}
fn main()->u8{let mut c=Counter::new(); c.bump(); c.get()}
"#,
    )
    .unwrap();
}
#[test]
fn generic_family() {
    check(
        "generic",
        r#"
spec type Cell<T> {fn new(value:T)->Self;fn take(self)->T;}
struct Repr<T>{value:T}
impl<U> Cell<U> for Repr<U> {
 fn new(value:U)->Self {Self{value}}
 fn take(self)->U {self.value}
}
fn main()->u8 {let cell:Cell<u8>=Cell::new(3);cell.take()}
"#,
    )
    .unwrap();
}
#[test]
fn associated() {
    check("associated",r#"
spec type Counter {type Value; const INITIAL:Self::Value; fn new()->Self; fn take(self)->Self::Value;}
struct Repr {n:u8}
impl Counter for Repr {
 type Value=u8;
 const INITIAL:u8=7;
 fn new()->Self{Self{n:Self::INITIAL}}
 fn take(self)->u8{self.n}
}
fn main()->Counter::Value{Counter::new().take()}
"#).unwrap();
}
#[test]
fn proof_result() {
    check("proof",r#"
spec type Math {fn next(n:u8)->(out:u8,@(out==n.wrapping_add(1)));}
struct Repr {n:u8}
impl Math for Repr {fn next(value:u8)->(result:u8,@(result==value.wrapping_add(1))){let out=value.wrapping_add(1);(out,_)}}
fn main()->u8{let (out,p)=Math::next(3);out}
"#).unwrap();
}
#[test]
fn split_module() {
    check("split",r#"
mod api {pub spec type Counter {fn new()->Self;fn get(&self)->u8;}}
mod body {struct Repr{n:u8} impl crate::api::Counter for Repr{fn new()->Self{Self{n:1}}fn get(&self)->u8{self.n}}}
fn main()->u8{let c=api::Counter::new();c.get()}
"#).unwrap();
}
#[test]
#[doc = "spec: 1.29:3"]
fn generic_family_uniqueness_and_instantiation_policy() {
    for (i, code) in [
        "spec type S<T>{}struct R<T>{n:T}impl S<u8> for R<u8>{}",
        "spec type S<T>{}struct R<T>{n:T}impl<T:Logical> S<T> for R<T>{}",
        "spec type S<T>{}struct R<T>{n:T}impl<T> S<T> for R<T>{}impl<U> S<U> for R<U>{}",
        "spec type S<T,U>{}struct R<T,U>{t:T,u:U}impl<T,U> S<U,T> for R<T,U>{}",
        "spec type S<T>{fn f(x:T)->T;}struct R<T>{n:T}impl<T> S<T> for R<T>{}",
    ]
    .iter()
    .enumerate()
    {
        let error = check(&format!("family{i}"), code).err().unwrap();
        assert!(error.contains("L0511"), "{error}");
    }
    let source =
        "spec type S<T>{fn f()->@(1==0);}struct R<T>{n:T}impl<T> S<T> for R<T>{fn f()->@(1==0){_}}";
    check("unused_generic_false_body", source).unwrap();
    let error = check(
        "instantiated_generic_false_body",
        &format!("{source}fn use_it()->@(1==0){{S::<u8>::f()}}"),
    )
    .err()
    .unwrap();
    assert!(error.contains("L0230"), "{error}");
}
#[test]
fn nominal_self_inputs_results_and_enum_representations() {
    check("self_forms",r#"
spec type S {fn new()->Self;fn identity(value:Self)->Self;fn read(value:&Self)->u8;fn pair()->(Self,u8);}
enum R{Only(u8)}
impl S for R {
 fn new()->Self{R::Only(7)}
 fn identity(value:Self)->Self{value}
 fn read(value:&Self)->u8{match value{R::Only(n)=>n}}
 fn pair()->(Self,u8){(Self::new(),1)}
}
fn main()->u8{let (s,n)=S::pair();let s=S::identity(s);S::read(&s)}
"#).unwrap();
}
#[test]
fn associated_type_and_spec_names_cannot_be_smuggled() {
    for (i, code) in [
        "struct R{}impl R{type X=u8;}",
        "spec type S<T>{type X;}struct R<T>{n:T}impl<T>S<T> for R<T>{type X=T;}",
        "spec type S{fn f(x:Vec<Self>)->();}struct R{}impl S for R{fn f(x:Vec<Self>)->(){()}}",
        "spec type S{} struct R{n:u8}impl S for R{}fn forge()->S{S{__locus_repr:R{n:1}}}",
    ]
    .iter()
    .enumerate()
    {
        assert!(check(&format!("smuggle{i}"), code).is_err(), "{code}");
    }
}
#[test]
fn generic_rust_and_interpreter_agree() {
    let code = r#"
spec type Cell<T>{fn new(x:T)->Self;fn take(self)->T;}
struct Storage<T>{x:T}
impl<T> Cell<T> for Storage<T>{fn new(x:T)->Self{Self{x}}fn take(self)->T{self.x}}
pub fn answer()->u8{let c:Cell<u8>=Cell::new(42);c.take()}
"#;
    let checked = check("generic_rust", code).unwrap();
    let function = checked
        .checked
        .functions
        .iter()
        .find(|f| f.0.ends_with("_answer"))
        .expect("answer")
        .1;
    let a = locus::exec::CheckInterpreter::new(checked.checked.session.program(), 10000)
        .with_lending(checked.checked.session.lending())
        .call(function, vec![])
        .unwrap();
    let b = locus::erased::Interpreter::new(checked.checked.session.erased(), 10000)
        .call(function, vec![])
        .unwrap();
    assert_eq!(a, b);
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("spec_revision_generic_rust");
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{assert_eq!(answer(),42);}}"),
    )
    .unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("run"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("run"))
            .status()
            .unwrap()
            .success()
    );
}
#[test]
fn logical_definition_unfolds_through_adapter() {
    check(
        "logical_unfold",
        r#"
spec type Math{logic fn identity(n:Int)->Int;}
struct R{}
impl Math for R{logic fn identity(n:Int)->Int{n}}
fn theorem(n:Int)->@(Math::identity(n)==n){fold!(Math::identity,prove!(n==n))}
"#,
    )
    .unwrap();
}
#[test]
fn realization_is_owned_by_defining_package() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("spec_revision_packages");
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["owner/src", "consumer/src"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers=['owner','consumer']\nresolver='3'\n",
    )
    .unwrap();
    std::fs::write(root.join("owner/Cargo.toml"),"[package]\nname='spec_owner'\nversion='0.1.0'\nedition='2024'\n[package.metadata.locus]\nlib='lib.lc'\n").unwrap();
    std::fs::write(root.join("owner/src/lib.rs"), "").unwrap();
    std::fs::write(
        root.join("owner/lib.lc"),
        "pub spec type S{fn f()->u8;}struct R{}impl S for R{fn f()->u8{7}}",
    )
    .unwrap();
    std::fs::write(root.join("consumer/Cargo.toml"),"[package]\nname='consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nowner={package='spec_owner',path='../owner'}\n").unwrap();
    std::fs::write(root.join("consumer/src/lib.rs"), "").unwrap();
    let entry = root.join("consumer/export.lc");
    std::fs::write(&entry, "use owner::S;fn main()->u8{S::f()}").unwrap();
    locus::project::Build::new(&entry)
        .offline(true)
        .check()
        .unwrap_or_else(|e| panic!("diagnostics: {:?}", e.diagnostics));
    // A downstream package cannot supply a missing realization, even if signatures match.
    std::fs::write(root.join("owner/lib.lc"), "pub spec type S{fn f()->u8;}").unwrap();
    std::fs::write(&entry, "use owner::S;struct R{}impl S for R{fn f()->u8{7}}").unwrap();
    let failure = locus::project::Build::new(&entry)
        .offline(true)
        .check()
        .err()
        .unwrap();

    let error = failure.to_string();
    assert!(error.contains("same Cargo package"), "{error}");
}

#[test]
fn implementation_methods_call_each_other_and_mutate_once() {
    check("method_calls",r#"
spec type Counter{fn new()->Self;fn read(&self)->u8;fn bumped(&mut self)->u8;fn twice(&mut self)->u8;}
struct R{n:u8}
impl Counter for R{
 fn new()->Self{Self{n:0}}
 fn read(&self)->u8{self.n}
 fn bumped(&mut self)->u8{self.n=self.n+1;self.read()}
 fn twice(&mut self)->u8{let first=self.bumped();self.bumped()}
}
fn main()->u8{let mut c=Counter::new();c.twice()}
"#).unwrap();
}
#[test]
fn dependency_spec_uses_its_native_type_and_hides_generated_helpers() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("spec_revision_native_dependency");
    let _ = std::fs::remove_dir_all(&root);
    for d in ["owner/src", "consumer/src"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers=['owner','consumer']\nresolver='3'\n",
    )
    .unwrap();
    std::fs::write(root.join("owner/Cargo.toml"),"[package]\nname='opaque_owner'\nversion='0.1.0'\nedition='2024'\n[package.metadata.locus]\nlib='lib.lc'\n[[package.metadata.locus.targets]]\nname='api'\nentry='export.lc'\nrust-module='api'\n").unwrap();
    std::fs::write(
        root.join("owner/src/lib.rs"),
        "pub mod api{include!(\"generated.rs\");}",
    )
    .unwrap();
    std::fs::write(root.join("owner/lib.lc"),"pub spec type Counter{fn new()->Self;fn take(self)->u8;}struct Private{n:u8}impl Counter for Private{fn new()->Self{Self{n:7}}fn take(self)->u8{self.n}}").unwrap();
    std::fs::write(root.join("owner/export.lc"), "pub use crate::Counter;").unwrap();
    std::fs::write(root.join("consumer/Cargo.toml"),"[package]\nname='opaque_consumer'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nrenamed={package='opaque_owner',path='../owner'}\n").unwrap();
    std::fs::write(root.join("consumer/src/main.rs"),"include!(\"generated.rs\");fn main(){assert_eq!(answer(),7);let value:renamed::api::Counter=make();assert_eq!(value.take(),7);}").unwrap();
    std::fs::write(root.join("consumer/export.lc"),"use renamed::Counter;pub fn answer()->u8{Counter::new().take()}pub fn make()->Counter{Counter::new()}").unwrap();
    let owner = locus::project::Build::new(root.join("owner/export.lc"))
        .offline(true)
        .rust()
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("owner/src/generated.rs"), owner).unwrap();
    let source = locus::project::Build::new(root.join("consumer/export.lc"))
        .offline(true)
        .rust()
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(source.contains("::renamed::api::Counter"), "{source}");
    assert!(!source.contains("__locus_spec_"), "{source}");
    assert!(!source.contains("struct Private"), "{source}");
    std::fs::write(root.join("consumer/src/generated.rs"), source).unwrap();
    let out = std::process::Command::new("cargo")
        .args([
            "run",
            "--offline",
            "--quiet",
            "-p",
            "opaque_consumer",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
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
fn self_is_the_representation_but_the_explicit_spec_name_is_not() {
    check("explicit_representation","spec type S{fn new()->S;}struct R{n:u8}impl S for R{fn new()->R{R{n:7}}}fn main()->S{S::new()}").unwrap();
    assert!(
        check(
            "distinct_spec_name",
            "spec type S{fn new()->Self;}struct R{n:u8}impl S for R{fn new()->S{S{n:7}}}"
        )
        .err()
        .unwrap()
        .contains("L0511")
    );
    assert!(check("cross_family","spec type S<T>{fn other()->S<u8>;}struct R<T>{n:T}impl<T>S<T> for R<T>{fn other()->Self{loop{}}}").err().unwrap().contains("L0510"));
}

#[test]
fn generic_model_impl_is_rejected_instead_of_silently_dropped() {
    let error = check(
        "generic_model",
        "struct R<T>{n:T}impl<T> Model for R<T>{type Logic=Int;logic fn model(&self)->Int{0}}",
    )
    .err()
    .unwrap();
    assert!(error.contains("generic Model implementations"), "{error}");
}

#[test]
#[doc = "spec: 1.29:5"]
fn spec_adapters_preserve_scoped_evidence_and_logical_observations() {
    check(
        "scoped_evidence",
        r#"
spec type Claims {
    fn certify(n: u8, proof: @(n > 0)) -> Option<@(n > 0)>;
    logic fn positive(n: &&u8) -> Prop;
}
struct Representation {}
impl Claims for Representation {
    fn certify(value: u8, evidence: @(value > 0)) -> Option<@(value > 0)> {
        Some(evidence)
    }
    logic fn positive(n: &&u8) -> Prop { prop!(n > 0) }
}
fn client(n: u8, proof: @(n > 0)) -> @(n > 0) {
    match Claims::certify(n, proof) {
        Option::Some(evidence) => evidence,
        Option::None => proof,
    }
}
fn observed(n: u8, proof: @(n > 0)) -> @Claims::positive(n) {
    let reference = &n;
    let evidence: @Claims::positive(&reference) = fold!(Claims::positive, proof);
    evidence
}
"#,
    )
    .unwrap();
    let error = check(
        "scoped_wrong_contract",
        r#"
spec type Claims { fn certify(n: u8, proof: @(n > 0)) -> Option<@(n > 0)>; }
struct Representation {}
impl Claims for Representation {
    fn certify(value: u8, evidence: @(value > 0)) -> Option<@(value == 0)> { None }
}
"#,
    )
    .err()
    .unwrap();
    assert!(error.contains("L0511"), "{error}");
    let error = check(
        "scoped_wrong_snapshot",
        r#"
spec type Claims { fn certify(n: u8, proof: @(n > 0)) -> Option<@(n > 0)>; }
struct Representation {}
impl Claims for Representation {
    fn certify(value: u8, evidence: @(value > 0)) -> Option<@(value > 0)> { Some(evidence) }
}
fn bad(n: u8, other: u8, proof: @(n > 0)) -> @(other > 0) {
    match Claims::certify(n, proof) { Option::Some(evidence) => evidence, Option::None => proof }
}
"#,
    )
    .err()
    .unwrap();
    assert!(error.contains("L02"), "{error}");
}
