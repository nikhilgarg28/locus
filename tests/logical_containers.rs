//! Physical containers retain their shape and effects around erased payloads.
use locus::{
    elab::{Elaborated, elaborate},
    erased::{self, Outcome, Value},
    source::SourceMap,
};
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("logical_containers.lc", text);
    let source = sources.get(id);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    elaborate(source, &parsed.program)
}
fn accepts(text: &str, expected: Value, assertion: &str) {
    let checked = check(text);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    erased::check_module(checked.session.erased()).unwrap();
    let function = checked.function("run").unwrap();
    let a = locus::exec::CheckInterpreter::new(checked.session.program(), 10000)
        .with_lending(checked.session.lending())
        .call(function, vec![])
        .unwrap();
    let b = erased::Interpreter::new(checked.session.erased(), 10000)
        .call(function, vec![])
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(b, Outcome::Value(expected));
    let rust = erased::print_module(checked.session.erased());
    let dir = std::env::temp_dir().join(format!(
        "locus-logical-containers-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{{assertion}}}"),
    )
    .unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("main"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("main"))
            .status()
            .unwrap()
            .success()
    );
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn vectors_of_int_keep_length_push_and_store() {
    accepts(
        "fn run()->Vec<Int>{let mut xs:Vec<Int>=Vec::from([1 as Int,2 as Int]);xs.push(3 as Int);xs[1]=4 as Int;xs}",
        Value::Buffer(vec![Value::Ghost; 3]),
        "assert_eq!(run().len(),3);",
    );
}
#[test]
#[doc = "spec: 1.3:1"]
fn vectors_of_logical_bool_preserve_marker_layout() {
    accepts(
        "fn run()->Vec<Bool>{let mut xs:Vec<Bool>=Vec::from([logic{true},logic{false}]);xs.push(logic{true});xs[1]=logic{true};xs}",
        Value::Buffer(vec![Value::Ghost; 3]),
        "assert_eq!(run().len(),3);",
    );
}
#[test]
fn arrays_and_slices_of_int_allow_checked_logical_access() {
    accepts(
        "fn first(xs:&[Int],present:@(0<xs.len()))->Int{xs[0]} fn run()->usize{let xs:[Int;2]=[1 as Int,2 as Int];let observed=first(&xs,prove!(0<xs.len()));xs.len()}",
        Value::Int(locus::kernel::PointerWidth::HOST.usize(), 2),
        "assert_eq!(run(),2);",
    );
}
#[test]
#[doc = "spec: 1.3:1"]
fn logical_recursive_payloads_keep_a_physical_array() {
    accepts(
        "#[derive(Logical)] enum Peano{Zero,Succ(Peano)} fn run()->[Peano;1]{[Peano::Succ(Peano::Zero)]}",
        Value::Buffer(vec![Value::Ghost]),
        "assert_eq!(run().len(),1);",
    );
}
#[test]
fn vectors_of_evidence_keep_the_proof_marker() {
    accepts(
        "fn run()->Vec<@(true)>{Vec::from([True::Intro])}",
        Value::Buffer(vec![Value::Proved]),
        "assert_eq!(run().len(),1);",
    );
}
#[test]
#[doc = "spec: 2.35:5, 2.39:3"]
fn logical_payload_construction_keeps_ordinary_call_effects() {
    accepts(
        "fn effect(n:&mut u8)->Int{*n=(*n).wrapping_add(1);7 as Int} fn run()->u8{let mut n:u8=0;let mut xs:Vec<Int>=Vec::from([effect(&mut n)]);xs.push(effect(&mut n));xs[0]=effect(&mut n);let observed=xs[0];n}",
        Value::u8(3),
        "assert_eq!(run(),3);",
    );
}
#[test]
#[doc = "spec: 1.26:6"]
fn logical_elements_cannot_be_extracted_as_runtime_data() {
    for source in [
        "fn bad(xs:&[Bool],present:@(0<xs.len()))->bool{xs[0]}",
        "fn bad(xs:&[Int],present:@(0<xs.len()))->usize{xs[0]}",
        "fn bad(xs:&[Bool],present:@(0<xs.len()))->u8{if xs[0]{1}else{0}}",
        "fn bad()->Vec<bool>{Vec::from([logic{true}])}",
        "fn bad()->[bool;1]{[logic{true}]}",
    ] {
        let result = check(source);
        assert!(!result.is_success(), "accepted {source}");
    }
}

#[test]
fn inferred_logical_bool_elements_are_not_runtime_bool() {
    accepts(
        "fn run()->Vec<Bool>{let xs=Vec::from([logic{true}]);xs}",
        Value::Buffer(vec![Value::Ghost]),
        "assert_eq!(run().len(),1);",
    );
}
#[test]
fn slices_of_logical_bool_can_be_borrowed_and_observed() {
    accepts(
        "fn first<'a>(xs:&'a[Bool],present:@(0<xs.len()))->&'a Bool{&xs[0]} fn run()->usize{let xs:[Bool;1]=[logic{true}];let r=first(&xs,prove!(0<xs.len()));let observed=logic{*r};xs.len()}",
        Value::Int(locus::kernel::PointerWidth::HOST.usize(), 1),
        "assert_eq!(run(),1);",
    );
}

#[test]
#[doc = "spec: 1.26:6"]
fn mixed_tuple_payloads_keep_only_runtime_fields() {
    accepts(
        "fn run()->Vec<(Bool,u8)>{Vec::from([(logic{true},7u8)])}",
        Value::Buffer(vec![Value::Tuple(vec![Value::Ghost, Value::u8(7)])]),
        "assert_eq!(run()[0].1,7);",
    );
}

#[test]
fn stored_evidence_can_be_read_as_evidence() {
    accepts(
        "fn run()->u8{let xs:Vec<@(true)>=Vec::from([True::Intro]);let p:@(true)=xs[0];7}",
        Value::u8(7),
        "assert_eq!(run(),7);",
    );
}
