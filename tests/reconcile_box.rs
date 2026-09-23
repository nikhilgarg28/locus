use locus::{
    elab::{Elaborated, Options, elaborate_with_options},
    preview::{Feature, Status},
    source::SourceMap,
};
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("box.lc", text);
    let file = sources.get(id);
    let parsed = locus::parser::parse(file);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    for f in [Feature::HeapViews, Feature::LogicalData] {
        if f.status() == Status::Preview {
            options.previews.enable(f.name()).unwrap();
        }
    }
    elaborate_with_options(file, &parsed.program, &options)
}
fn accepts(text: &str, main: &str) {
    let result = check(text);
    assert!(result.is_success(), "{:?}", result.diagnostics);
    locus::erased::check_module(result.session.erased()).unwrap();
    if let Some(function) = result.function("run") {
        let checked = locus::exec::CheckInterpreter::new(result.session.program(), 10000)
            .with_lending(result.session.lending())
            .call(function, vec![])
            .unwrap();
        let erased = locus::erased::Interpreter::new(result.session.erased(), 10000)
            .call(function, vec![])
            .unwrap();
        assert_eq!(checked, erased);
    }
    let rust = locus::erased::print_module(result.session.erased());
    let dir = std::env::temp_dir().join(format!(
        "locus-box-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.rs");
    std::fs::write(&path, format!("{rust}\nfn main() {{{main}}}")).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(&path)
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
fn box_payload_and_repeated_copy_read() {
    accepts(
        "fn run()->u8 {let x=Box::new(7u8); let first=*x; first.wrapping_add(*x)}",
        "assert_eq!(run(),14);",
    );
}
#[test]
fn logical_payload_keeps_physical_box() {
    accepts(
        "fn boxed()->Box<Int> {Box::new(7 as Int)} fn run()->u8 {let x=boxed(); let observed=logic { *x }; 4}",
        "let _b=boxed(); assert_eq!(run(),4);",
    );
}
#[test]
#[doc = "spec: 2.36:4"]
fn runtime_recursive_enum_has_box_layout() {
    accepts(
        "enum List {Nil,Cons(u8,Box<List>)} fn run()->u8 {let xs=List::Cons(7,Box::new(List::Nil)); match xs {List::Nil=>0,List::Cons(head,tail)=>head}}",
        "assert_eq!(run(),7);",
    );
}
#[test]
fn box_never_derives_logical_or_copy() {
    for source in [
        "#[derive(Logical)] struct Bad {item:Box<Int>}",
        "#[derive(Copy)] struct Bad {item:Box<u8>}",
        "enum Bad {Next(Bad)}",
    ] {
        let result = check(source);
        assert!(!result.is_success(), "accepted {source}");
    }
}
#[test]
fn logical_payload_cannot_control_runtime() {
    let result = check("fn bad(x:Box<Int>)->u8 {if *x > 0 {1}else{2}}");
    assert!(!result.is_success());
}
#[test]
fn structural_model_observes_boxed_descendants() {
    let result = check(
        "enum List {Nil,Cons(u8,Box<List>)} logic fn length(xs:&List)->Int {match xs {List::Nil=>0,List::Cons(head,tail)=>1+length(&*tail)}} fn run()->u8 {let xs=List::Cons(7,Box::new(List::Nil));let count=length(&xs); 7}",
    );
    assert!(result.is_success(), "{:?}", result.diagnostics);
}
#[test]
fn logical_boolean_payload_stays_erased_inside_physical_box() {
    accepts(
        "logic fn flag()->Bool {true} fn boxed()->Box<Bool> {Box::new(flag())} fn run()->u8 {let x=boxed(); let observed=logic { *x }; 8}",
        "let _b=boxed();assert_eq!(run(),8);",
    );
}
#[test]
fn effectful_logical_payload_call_still_executes() {
    accepts(
        "fn effect(n:&mut u8)->Int {*n=(*n).wrapping_add(1);7 as Int} fn run()->u8 {let mut n:u8=1;let boxed=Box::new(effect(&mut n));n}",
        "assert_eq!(run(),2);",
    );
}

#[test]
fn recursive_logical_model_calls_are_not_executed() {
    accepts(
        include_str!("corpus/target/library_runtime_list.lc"),
        "assert_eq!(run(), 7);",
    );
}

#[test]
fn recursive_runtime_enum_preserves_erased_payload_layout() {
    accepts(
        "enum List { Nil, Cons(Bool, Box<List>) }
      fn run() -> u8 {
        let xs = List::Cons(logic { true }, Box::new(List::Nil));
        match xs { List::Nil => 0, List::Cons(flag, tail) => 8 }
      }",
        "assert_eq!(run(), 8);",
    );
}
