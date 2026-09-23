use locus::{
    elab::{Elaborated, Options, elaborate_with_options},
    preview::{Feature, Status},
    source::SourceMap,
};
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("shared.lc", text);
    let file = sources.get(id);
    let parsed = locus::parser::parse(file);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    for feature in [
        Feature::HeapViews,
        Feature::LogicalSplit,
        Feature::LogicalData,
    ] {
        if feature.status() == Status::Preview {
            options.previews.enable(feature.name()).unwrap();
        }
    }
    elaborate_with_options(file, &parsed.program, &options)
}
fn accepts(text: &str, main: &str) {
    let result = check(text);
    assert!(result.is_success(), "{:?}", result.diagnostics);
    locus::erased::check_module(result.session.erased()).unwrap();
    let rust = locus::erased::print_module(result.session.erased());
    let dir = std::env::temp_dir().join(format!(
        "locus-shared-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("case")
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.rs");
    std::fs::write(&path, format!("{rust}\nfn main() {{ {main} }}")).unwrap();
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
fn rejects(text: &str) {
    let result = check(text);
    assert!(!result.is_success(), "accepted unsafe reference program");
    assert!(
        result.diagnostics.iter().any(|d| d.code == "L0286"),
        "{:?}",
        result.diagnostics
    );
}
#[test]
#[doc = "spec: 2.38:1, 2.38:2"]
fn local_shared_reference() {
    accepts(
        "fn run()->u8 { let x:u8=7; let r=&x; *r }",
        "assert_eq!(run(),7);",
    );
}
#[test]
fn reference_result_uses_named_input_lifetime() {
    accepts(
        "fn identity<'a>(x:&'a u8)->&'a u8 { &x } fn run()->u8 { let x:u8=9; let r=identity(&x); *r }",
        "assert_eq!(run(),9);",
    );
}
#[test]
#[doc = "spec: 1.3:1, 1.26:2"]
fn reference_field_retains_its_referent() {
    accepts(
        "struct Holder<'a>{ item:&'a u8 } fn hold<'a>(x:&'a u8)->Holder<'a> { Holder { item:&x } } fn run()->u8 { let x:u8=11; let holder=hold(&x); *holder.item }",
        "assert_eq!(run(),11);",
    );
}
#[test]
fn a_local_reference_cannot_escape() {
    rejects("fn bad<'a>(x:&'a u8)->&'a u8 { let local:u8=4; &local }");
}
#[test]
fn nested_block_storage_cannot_escape() {
    rejects("fn bad()->u8 { let r={let x:u8=5; &x}; *r }");
}
#[test]
#[doc = "spec: 2.38:3, 2.38:4"]
fn write_before_last_use_is_rejected() {
    rejects("fn bad()->u8 { let mut x:u8=5; let r=&x; x=6; *r }");
}
#[test]
#[doc = "spec: 3.4:6"]
fn logical_observation_is_a_reference_use() {
    rejects(
        "fn bad()->u8 { let mut x:u8=5; let r=&x; x=6; let observation=logic { (*r) as Int }; x }",
    );
}
#[test]
fn last_use_before_write_is_allowed() {
    accepts(
        "fn run()->u8 { let mut x:u8=5; let r=&x; let old=*r; x=6; old.wrapping_add(x) }",
        "assert_eq!(run(),11);",
    );
}
#[test]
#[doc = "spec: 3.4:6"]
fn wrong_input_lifetime_cannot_escape() {
    rejects("fn bad<'a,'b>(x:&'a u8,y:&'b u8)->&'a u8 { &y }");
}

#[test]
fn lookup_returns_a_reference_into_input_storage() {
    accepts(
        "fn first<'a>(xs:&'a [u8],present:@(0 < xs.len()))->&'a u8 { &xs[0] } fn run()->u8 { let xs:[u8;2]=[17,19]; let r=first(&xs,prove!(0 < xs.len())); *r }",
        "assert_eq!(run(),17);",
    );
}
#[test]
fn explicit_return_preserves_reference_origin() {
    accepts(
        "fn identity<'a>(x:&'a u8)->&'a u8 { return &x; } fn run()->u8 { let x:u8=9; *identity(&x) }",
        "assert_eq!(run(),9);",
    );
}
#[test]
fn moving_a_holder_transports_its_reference() {
    accepts(
        "struct Holder<'a>{item:&'a u8} fn take<'a>(h:Holder<'a>)->&'a u8 {h.item} fn run()->u8 {let x:u8=13;let h=Holder{item:&x};let r=take(h);*r}",
        "assert_eq!(run(),13);",
    );
}
#[test]
fn input_lifetime_can_name_either_input() {
    accepts(
        "fn choose<'a>(x:&'a u8,y:&'a u8,b:bool)->&'a u8 {if b {&x}else{&y}} fn run()->u8{let x:u8=4;let y:u8=8;*choose(&x,&y,false)}",
        "assert_eq!(run(),8);",
    );
}
#[test]
fn parser_holds_input_and_returns_borrowed_lookup() {
    accepts(
        r#"
struct Parser<'a> { input: &'a [u8], present:@(0 < input.len()) }
fn parser<'a>(input:&'a [u8],present:@(0 < input.len()))->Parser<'a> { Parser {input:&input,present} }
fn first<'a>(p:Parser<'a>)->&'a u8 { &p.input[0] }
fn run()->u8 { let xs:[u8;2]=[21,22]; let p=parser(&xs,prove!(0 < xs.len())); let r=first(p); *r }
"#,
        "assert_eq!(run(),21);",
    );
}
#[test]
fn iteration_can_hold_a_shared_element_for_each_iteration() {
    accepts(
        r#"
fn total<'a>(xs:&'a [u8])->u8 { let mut out:u8=0; for i in 0..xs.len() {let r=&xs[i];out=out.wrapping_add(*r);} out }
fn run()->u8 {let xs:[u8;3]=[2,3,4]; total(&xs)}
"#,
        "assert_eq!(run(),9);",
    );
}
#[test]
#[doc = "spec: 2.38:5"]
fn lookup_borrows_non_copy_elements() {
    accepts(
        r#"
struct Item { value:u8 }
fn head<'a>(xs:&'a [Item],present:@(0 < xs.len()))->&'a Item { &xs[0] }
fn run()->u8 {let xs:[Item;1]=[Item{value:31}];let r=head(&xs,prove!(0 < xs.len()));r.value}
"#,
        "assert_eq!(run(),31);",
    );
}
#[test]
fn shared_lookup_prevents_later_mutation_before_use() {
    rejects(
        r#"
fn bad()->u8 {let mut xs:[u8;2]=[1,2];let r=&xs[0];xs[0]=3u8;*r}
"#,
    );
}
#[test]
fn native_helpers_preserve_distinct_element_erasure_shapes() {
    accepts(
        r#"
fn run()->u8 {let a:Vec<(bool,u8)>=Vec::from([(true,3)]);let b:Vec<(Bool,u8)>=Vec::from([(logic{true},4)]);a[0].1.wrapping_add(b[0].1)}
"#,
        "assert_eq!(run(),7);",
    );
}
#[test]
fn field_order_does_not_change_lifetime_argument_meaning() {
    accepts(
        r#"
struct Pair<'a,'b>{second:&'b u8,first:&'a u8}
fn pick<'x,'y>(p:Pair<'x,'y>)->&'x u8{p.first}
fn run()->u8{let x:u8=2;let y:u8=3;let p=Pair{second:&y,first:&x};*pick(p)}
"#,
        "assert_eq!(run(),2);",
    );
    rejects(
        "struct Pair<'a,'b>{second:&'b u8,first:&'a u8} fn bad<'x,'y>(p:Pair<'x,'y>)->&'x u8{p.second}",
    );
}
#[test]
fn a_shared_reference_to_non_copy_data_is_itself_copy() {
    accepts(
        r#"
struct Item{n:u8}
fn run()->u8{let value=Item{n:4};let r=&value;let other=r;r.n.wrapping_add(other.n)}
"#,
        "assert_eq!(run(),8);",
    );
}
#[test]
#[doc = "spec: 2.38:6"]
fn rust_independently_rejects_the_lifetime_counterexamples() {
    let cases = [
        "fn bad<'a>(_x:&'a u8)->&'a u8{let local=4;&local}",
        "fn bad()->u8{let r={let x=5;&x};*r}",
        "fn bad()->u8{let mut x=5;let r=&x;x=6;*r}",
        "fn bad()->u8{let mut x=5;let r=&x;x=6;let _observation=*r;x}",
        "fn bad<'a,'b>(_x:&'a u8,y:&'b u8)->&'a u8{y}",
        "fn bad()->u8{let mut xs=[1,2];let r=&xs[0];xs[0]=3;*r}",
        "struct Pair<'a,'b>{second:&'b u8,first:&'a u8} fn bad<'x,'y>(p:Pair<'x,'y>)->&'x u8{p.second}",
        "#[derive(Clone,Copy)] struct Item{n:u8} fn bad()->u8{let mut item=Item{n:2};let r=&item;let field=&r.n;item=Item{n:3};*field}",
        "struct Holder<'a>{item:&'a u8} fn bad<'a>(h:&mut Holder<'a>)->u8{let local=5;h.item=&local;0}",
        "struct Holder<'a>{item:&'a u8,n:u8} fn bad<'a>(x:&'a u8)->Holder<'a>{let mut h=Holder{item:x,n:0};let local=5;h.item=&local;h.n=3;h}",
        "fn bad()->u8{let mut value:u8=1;let reference=&value;while *reference>0 {value=0;} value}",
        "fn bad()->u8{let mut value:u8=1;let reference=&value;while {let _observed=*reference;value>0} {value=0;} value}",
        "struct Holder<'a>{value:&'a u8} fn transport<'a>(h:Holder<'a>)->Holder<'a>{h} fn bump(v:&mut u8){*v=v.wrapping_add(1);} fn bad()->u8{let mut value=1;let holder=Holder{value:&value};let carried=transport(holder);bump(&mut value);let _observed=*carried.value;value}",
        "enum Packet{Item(u8)} fn bad()->u8{let reference=match Packet::Item(5){Packet::Item(value)=>&value};*reference}",
    ];
    let dir = std::env::temp_dir().join(format!("locus-reference-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (i, source) in cases.iter().enumerate() {
        let file = dir.join(format!("case{i}.rs"));
        std::fs::write(&file, source).unwrap();
        let out = std::process::Command::new("rustc")
            .args(["--edition=2024", "--crate-type=lib"])
            .arg(&file)
            .arg("-o")
            .arg(dir.join("case.rlib"))
            .output()
            .unwrap();
        assert!(!out.status.success(), "Rust accepted {source}");
    }
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
fn a_field_borrow_uses_the_referent_not_the_pointer_holder() {
    rejects(
        "#[derive(Clone,Copy)] struct Item{n:u8} fn bad()->u8{let mut item=Item{n:2};let r=&item;let field=&r.n;item=Item{n:3};*field}",
    );
}
#[test]
fn mutating_a_holder_cannot_store_a_shorter_lived_reference() {
    rejects(
        "struct Holder<'a>{item:&'a u8} fn bad<'a>(h:&mut Holder<'a>)->u8{let local:u8=5;h.item=&local;0}",
    );
}
#[test]
fn a_second_assignment_keeps_the_new_reference_origin() {
    rejects(
        "struct Holder<'a>{item:&'a u8,n:u8} fn bad<'a>(x:&'a u8)->Holder<'a>{let mut h=Holder{item:&x,n:0};let local:u8=5;h.item=&local;h.n=3;h}",
    );
}
#[test]
fn shared_holder_keeps_the_reference_inside_it() {
    accepts(
        "struct Holder<'a>{item:&'a u8} fn run()->u8{let n:u8=5;let h=Holder{item:&n};let borrowed=&h;let r=borrowed.item;*r}",
        "assert_eq!(run(),5);",
    );
}
#[test]
#[doc = "spec: 1.26:4"]
fn optional_references_preserve_variant_specific_origins() {
    accepts(
        "enum Maybe<'a>{None,Some(&'a u8)} fn empty<'a>()->Maybe<'a>{Maybe::None} fn wrap<'a>(n:&'a u8)->Maybe<'a>{Maybe::Some(&n)} fn run()->u8{let n:u8=6;match wrap(&n){Maybe::None=>0,Maybe::Some(r)=>*r}}",
        "assert_eq!(run(),6);",
    );
}

#[test]
fn while_conditions_check_observation_permissions_on_every_back_edge() {
    rejects(
        "fn bad()->u8{let mut value:u8=1;let reference=&value;while *reference>0 {value=0;} value}",
    );
    rejects(
        "fn bad()->u8{let mut value:u8=1;let reference=&value;while {let observed=logic{(*reference) as Int};value>0} {value=0;} value}",
    );
}

#[test]
#[doc = "spec: 1.26:2"]
fn transported_wrapper_cannot_hide_an_erased_use_after_mutation() {
    rejects(
        r#"
        struct Holder<'a>{value:&'a u8}
        fn transport<'a>(holder:Holder<'a>)->Holder<'a>{holder}
        fn bump(value:&mut u8)->(){value=value.wrapping_add(1);}
        fn bad()->u8{
            let mut value:u8=1;
            let holder=Holder{value:&value};
            let carried=transport(holder);
            bump(&mut value);
            let observed=logic{(*carried.value) as Int};
            value
        }
    "#,
    );
}

#[test]
fn a_reference_to_match_payload_storage_cannot_outlive_the_arm() {
    rejects(
        "enum Packet{Item(u8)} fn bad()->u8{let reference=match Packet::Item(5){Packet::Item(value)=>&value};*reference}",
    );
}
