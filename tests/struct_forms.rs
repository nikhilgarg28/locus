use locus::{
    erased::{Outcome, Value},
    kernel::MachineInt,
    project,
};
use std::{path::PathBuf, process::Command};
fn check(tag: &str, code: &str) -> Result<project::Checked, String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("struct_forms_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.lc");
    std::fs::write(&path, code).unwrap();
    project::check(&path, &Default::default()).map_err(|e| e.to_string())
}
fn agrees(tag: &str, code: &str, expected: u8) {
    let checked = check(tag, code).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let f = checked
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("_demo"))
        .unwrap()
        .1;
    let a = locus::exec::CheckInterpreter::new(checked.checked.session.program(), 10000)
        .with_lending(checked.checked.session.lending())
        .call(f, vec![])
        .unwrap();
    let b = locus::erased::Interpreter::new(checked.checked.session.erased(), 10000)
        .call(f, vec![])
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a,
        Outcome::Value(Value::Int(MachineInt::U8, expected.into()))
    );
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("struct_forms_{tag}"));
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{assert_eq!(demo(),{expected});}}\n"),
    )
    .unwrap();
    let output = Command::new("rustc")
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
    assert!(Command::new(dir.join("run")).status().unwrap().success());
}
#[test]
#[doc = "spec: 1.96:4"]
#[doc = "spec: 1.4:3"]
#[doc = "spec: 1.96:7"]
fn positional_and_unit_construction_projection_mutation_and_patterns() {
    agrees(
        "basic",
        r#"
struct Byte(u8); struct Ready; struct Empty();
impl Byte {fn bump(&mut self){self.0=self.0+1;}}
pub fn demo()->u8 {
 let mut b=Byte(7);b.bump();let Byte(value)=b;
 let Ready=Ready;let Empty()=Empty();
 match Byte(value) {Byte(x)=>x}
}
"#,
        8,
    );
}
#[test]
fn generic_tuple_constructor_inference_and_self() {
    agrees(
        "generic",
        r#"
struct Pair<T>(T,T);
struct Byte(u8); impl Byte{fn new(n:u8)->Self{Self(n)}}
pub fn demo()->u8 {
 let pair=Pair(2u8,3u8);let Pair(a,b)=pair;
 let other:Pair<u8>=Pair(a,b);Byte::new(other.0+other.1).0
}
"#,
        5,
    );
}
#[test]
#[doc = "spec: 1.96:5"]
fn dependent_proof_fields_check_construct_and_destructure() {
    agrees(
        "proof",
        r#"
struct Nonzero(value:u8,@(value>0));
fn consume(n:u8,p:@(n>0))->u8{n}
pub fn demo()->u8 {let item=Nonzero(3,_);let Nonzero(n,p)=item;consume(n,p)}
"#,
        3,
    );
}
#[test]
fn unit_nominality_and_constructor_shapes_are_enforced() {
    for (i, source) in [
        "struct A;struct B;fn bad()->A{B}",
        "struct A;fn A(){}",
        "fn A(){}struct A(u8);",
        "mod m{pub struct A;}use m::A;fn A(){}",
        "struct A;fn bad()->A{A()}",
        "struct A();fn bad()->A{A}",
        "struct A(u8);fn bad()->A{A{__field0:3}}",
        "struct A{n:u8}fn bad()->A{A(3)}",
        "struct A(n:u8);fn bad()->u8{A(3).n}",
        "struct A(u8);struct B(u8);fn bad(){let B(n)=A(3);}",
        "struct A(n:u8,@(n>0));fn bad()->A{A(0,_)}",
        "struct A(n:u8,@(n>0));fn bad(){let mut a=A(3,_);a.0=0;}",
        "struct A(n:u8);fn bad(){let mut a=A(3);a.n=0;}",
    ]
    .iter()
    .enumerate()
    {
        assert!(
            check(&format!("bad{i}"), source).is_err(),
            "accepted {source}"
        );
    }
}
#[test]
#[doc = "spec: 1.96:5"]
fn private_tuple_fields_block_cross_module_construction_and_patterns() {
    for (i, body) in [
        "m::Hidden(2)",
        "{let m::Hidden(x)=m::make();m::make()}",
        "{let x=m::make();let n=x.0;m::make()}",
    ]
    .iter()
    .enumerate()
    {
        let code = format!(
            "mod m{{pub struct Hidden(u8);pub fn make()->Hidden{{Hidden(1)}}}} fn bad()->m::Hidden{{{body}}}"
        );
        assert!(check(&format!("private{i}"), &code).is_err());
    }
    agrees(
        "module",
        "mod m{pub struct Byte(pub u8);pub struct Ready;}pub fn demo()->u8{let m::Ready=m::Ready;let m::Byte(n)=m::Byte(9);n}",
        9,
    );
}

#[test]
fn tuple_representation_satisfies_a_spec() {
    agrees(
        "spec",
        r#"
spec type Byte {fn new(n:u8)->Self;fn get(&self)->u8;}
struct Repr(u8);
impl Byte for Repr {fn new(n:u8)->Self{Self(n)}fn get(&self)->u8{self.0}}
pub fn demo()->u8 {let b=Byte::new(7);b.get()}
"#,
        7,
    );
}
#[test]
fn struct_pattern_preserves_noncopy_moves_and_shared_borrow_rules() {
    for (i,source) in [
 "struct A(u8);struct Pair(A,A);fn f(){let p=Pair(A(1),A(2));let Pair(a,_)=p;let again=p.0;}",
 "struct A(u8);struct Pair(A,A);fn f(p:&Pair){let Pair(a,_)=p;}",
 ].iter().enumerate(){assert!(check(&format!("move{i}"),source).is_err(),"accepted {source}");}
    agrees(
        "partial",
        "struct A(u8);struct Pair(A,A);pub fn demo()->u8{let p=Pair(A(1),A(2));let Pair(a,_)=p;a.0+p.1.0}",
        3,
    );
}

#[test]
#[doc = "spec: 1.96:7"]
#[doc = "spec: 3.10:2"]
fn logical_structs_and_nested_positional_patterns_erase_correctly() {
    agrees(
        "logical",
        r#"
#[derive(Logical)] struct Limit(n:Int,@(n>=0));
#[derive(Logical)] struct Ready;
pub fn demo()->u8 {
 let item=logic {Limit(3,_)};
 let Limit(n,p)=item;
 let check:@(n>=0)=p;
 let Ready=logic {Ready};
 7
}
"#,
        7,
    );
    agrees(
        "nested",
        "struct Inner(u8);struct Outer(Inner);pub fn demo()->u8{let Outer(Inner(n))=Outer(Inner(8));n}",
        8,
    );
    for (tag, code) in [
        ("unnamed", "struct S(u8,@(__field0>0));"),
        ("duplicate", "struct S(n:u8,n:u8);"),
        ("refutable", "struct S(u8);fn f(){let S(3)=S(4);}"),
    ] {
        assert!(check(tag, code).is_err(), "accepted {code}");
    }
}

#[test]
fn public_tuple_exports_and_private_proof_fields_keep_the_rust_boundary() {
    let code = r#"
pub struct Byte(pub u8);
pub struct Ready;
pub struct Empty();
pub struct Positive(n:u8,@(n>0));
impl Positive {pub fn new()->Self{Self(3,_)}pub fn value(&self)->u8{self.0}}
"#;
    let checked = check("exports", code).unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("struct_forms_exports");
    for (n, body, success) in [
        (
            "good",
            "let b=Byte(9);assert_eq!(b.0,9);let Ready=Ready;let Empty()=Empty();let p=Positive::new();assert_eq!(p.value(),3);",
            true,
        ),
        ("read", "let p=Positive::new();let _=p.0;", false),
        (
            "construct",
            "let p=Positive::new();let Positive(n,e)=p;let _=Positive(0,e);",
            false,
        ),
    ] {
        let file = dir.join(format!("{n}.rs"));
        std::fs::write(&file, format!("{rust}\nfn main(){{{body}}}")).unwrap();
        let out = Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings"])
            .arg(&file)
            .arg("-o")
            .arg(dir.join(n))
            .output()
            .unwrap();
        assert_eq!(
            out.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        if success {
            assert!(Command::new(dir.join(n)).status().unwrap().success());
        }
    }
    let invalid = check("export_leak", "pub struct Bad(pub n:u8,pub @(n>0));").unwrap();
    assert!(project::rust(invalid).is_err());
}

#[test]
fn tuple_struct_models_borrows_and_effectful_logical_producers() {
    agrees(
        "borrowed_fields",
        "struct Pair(u8,u8);fn sum(p:&Pair)->u8{let Pair(a,b)=p;a+b}pub fn demo()->u8{let pair=Pair(2,3);sum(&pair)}",
        5,
    );
    agrees(
        "derived_model",
        r#"
#[derive(Model)]struct Byte(u8);
fn prove_value(b:&Byte)->@(b.0>=0){_}
pub fn demo()->u8{let b=Byte(9);let p=prove_value(&b);b.0}
"#,
        9,
    );
    agrees(
        "logical_effect",
        r#"
#[derive(Logical)]struct ProofOnly(p:@(1==1));
fn mutate(n:&mut u8)->ProofOnly{n=9;logic{ProofOnly(_)}}
pub fn demo()->u8{let mut n:u8=0;let ProofOnly(p)=mutate(&mut n);n}
"#,
        9,
    );
}
