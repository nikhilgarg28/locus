#[path = "common/known_bug.rs"]
mod expected_failure;
use locus::project;
use std::path::PathBuf;
fn check(tag: &str, code: &str) -> Result<project::Checked, String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("bounds_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.lc");
    std::fs::write(&path, code).unwrap();
    project::check(&path, &Default::default()).map_err(|e| e.to_string())
}
fn run(tag: &str, code: &str, expected: &str) {
    let c = check(tag, code).unwrap();
    let m = c.checked.session.erased();
    locus::erased::check_module(m).unwrap();
    let f = c
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("run"))
        .unwrap()
        .1;
    let v = locus::erased::Interpreter::new(m, 10000)
        .call(f, vec![])
        .unwrap();
    let before_erasure = locus::exec::CheckInterpreter::new(c.checked.session.program(), 10000)
        .with_lending(c.checked.session.lending())
        .call(f, vec![])
        .unwrap();
    assert_eq!(before_erasure, v);
    assert_eq!(v.debug(m), expected);
    let public_source = if code.contains("pub fn run()") {
        code.to_owned()
    } else {
        code.replace("fn run()", "pub fn run()")
    };
    let tag = format!("{tag}_compiled");
    let checked = check(&tag, &public_source).unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("bounds_{tag}"));
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{assert_eq!(run(),{expected});}}"),
    )
    .unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("app"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{tag}: {}\n{rust}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("app"))
            .status()
            .unwrap()
            .success()
    );
}
#[test]
#[doc = "spec: 1.31:30"]
#[doc = "spec: 1.31:31"]
fn inline_where_and_static_dispatch() {
    for (i, bound) in ["<T:Read>(x:&T)->u8", "<T>(x:&T)->u8 where T:Read,"]
        .iter()
        .enumerate()
    {
        run(
            &format!("basic{i}"),
            &format!(
                "trait Read{{fn read(&self)->u8;}}struct S{{}}impl Read for S{{fn read(&self)->u8{{7}}}}impl S{{fn read(&self)->u8{{99}}}}fn get{bound}{{x.read()}}fn run()->u8{{let s=S{{}};get(&s)}}"
            ),
            "7",
        );
    }
}
#[test]
#[doc = "spec: 1.31:35"]
#[doc = "spec: 3.12:2"]
fn static_method_and_proof_slots() {
    run(
        "proof",
        r#"
 trait Step{fn next(n:u8,room:@(n<255))->(out:u8,@(out==n+1));}
 struct S{}impl Step for S{fn next(n:u8,room:@(n<255))->(out:u8,@(out==n+1)){let out=n+1;(out,_)}}
 fn advance<T:Step>(n:u8,p:@(n<255))->(out:u8,@(out==n+1)){T::next(n,p)}
 fn run()->u8{let(out,p)=advance::<S>(7,prove!(7<255));out}
 "#,
        "8",
    );
}
#[test]
#[doc = "spec: 1.31:32"]
fn associated_types() {
    run(
        "assoc",
        r#"
 trait Source{type Item;fn next(&self)->Self::Item;}struct S{}impl Source for S{type Item=u8;fn next(&self)->u8{9}}
 fn get<T:Source<Item=u8>>(x:&T)->T::Item{x.next()}
 fn run()->u8{let s=S{};get(&s)}
 "#,
        "9",
    );
    run(
        "assoc_open",
        r#"
 trait Source{type Item;fn next(&self)->Self::Item;}struct S{}impl Source for S{type Item=u8;fn next(&self)->u8{9}}
 fn get<T:Source>(x:&T)->T::Item{x.next()}
 fn run()->u8{let s=S{};get(&s)}
 "#,
        "9",
    );
}
#[test]
#[doc = "spec: 1.31:33"]
fn generic_impl() {
    run(
        "family",
        r#"
 trait Read{fn read(&self)->u8;}struct S{}impl Read for S{fn read(&self)->u8{8}}
 struct Wrapper<T>{inner:T}impl<T:Read> Read for Wrapper<T>{fn read(&self)->u8{self.inner.read()}}
 fn get<T:Read>(x:&T)->u8{x.read()}
 fn run()->u8{let w:Wrapper<S>=Wrapper{inner:S{}};get(&w)}
 "#,
        "8",
    );
}
#[test]
fn bounds_reject_missing_ambiguous_and_hidden_capabilities() {
    for (i, body) in [
        "fn get<T:Read>(x:&T)->u8{x.read()}fn run()->u8{get(&Bad{})}",
        "fn get<T>(x:&T)->u8{x.read()}fn run()->u8{let s=S{};get(&s)}",
        "fn get<T:Read>(x:&T)->u8{x.secret()}fn run()->u8{let s=S{};get(&s)}",
        "fn get<T:Read+Other>(x:&T)->u8{x.read()}fn run()->u8{let s=S{};get(&s)}",
        "fn get<T:Read>(x:&T)->u8{x.read()}fn leak<T>(x:&T)->u8{get(x)}fn run()->u8{leak(&S{})}",
    ]
    .iter()
    .enumerate()
    {
        let code = format!(
            "trait Read{{fn read(&self)->u8;}}trait Other{{fn read(&self)->u8;}}struct S{{}}struct Bad{{}}impl Read for S{{fn read(&self)->u8{{1}}}}impl Other for S{{fn read(&self)->u8{{2}}}}impl S{{fn secret(&self)->u8{{3}}}}{body}"
        );
        let e = check(&format!("bad{i}"), &code).err().expect("must reject");
        assert!(e.contains("L0281"), "{e}");
    }
}

#[test]
fn projection_bounds_and_explicit_qualification() {
    run(
        "projection",
        r#"
 trait Read{fn read(&self)->u8;}struct Item{}impl Read for Item{fn read(&self)->u8{12}}
 trait Source{type Item;fn item(&self)->Self::Item;}struct S{}impl Source for S{type Item=Item;fn item(&self)->Item{Item{}}}
 fn read<T:Read>(x:&T)->u8{x.read()}
 fn get<S>(source:&S)->u8 where S:Source,S::Item:Read {let item=source.item();read(&item)}
 fn run()->u8{let s=S{};get(&s)}
 "#,
        "12",
    );
    run(
        "qualified",
        r#"
 trait A{fn read(&self)->u8;}trait B{fn read(&self)->u8;}
 struct S{}impl A for S{fn read(&self)->u8{1}}impl B for S{fn read(&self)->u8{2}}
 fn get<T:A+B>(x:&T)->u8{<T as B>::read(&*x)}fn run()->u8{let s=S{};get(&s)}
 "#,
        "2",
    );
}
#[test]
#[doc = "spec: 1.31:34"]
fn conditional_inherent_methods_and_renamed_family_binders() {
    run(
        "conditional",
        r#"
 trait Read{fn read(&self)->u8;}struct S{}struct Bad{}impl Read for S{fn read(&self)->u8{4}}
 struct Holder<A>{inner:A}
 impl<T> Holder<T>{fn read(&self)->u8 where T:Read {self.inner.read()}fn tag(&self)->u8{3}}
 fn run()->u8{let good:Holder<S>=Holder{inner:S{}};let bad:Holder<Bad>=Holder{inner:Bad{}};good.read()+bad.tag()}
 "#,
        "7",
    );
    run(
        "renamed",
        r#"
 trait Read{fn read(&self)->u8;}struct S{}impl Read for S{fn read(&self)->u8{8}}
 struct Holder<A>{inner:A}impl<T:Read> Read for Holder<T>{fn read(&self)->u8{self.inner.read()}}
 fn get<T:Read>(x:&T)->u8{x.read()}fn run()->u8{let h:Holder<S>=Holder{inner:S{}};get(&h)}
 "#,
        "8",
    );
}
#[test]
fn where_bounds_on_structs_and_logical_functions() {
    run(
        "type_where",
        r#"
 trait Read{fn read(&self)->u8;}struct S{}impl Read for S{fn read(&self)->u8{5}}
 struct Holder<T> where T:Read {inner:T}
 fn run()->u8{let h:Holder<S>=Holder{inner:S{}};h.inner.read()}
 "#,
        "5",
    );
    check(
        "logical_where",
        r#"
 #[derive(Logical)] struct Pair<T> where T:Logical {a:T,b:T}
 logic fn duplicate<T>(n:T)->Pair<T> where T:Logical {Pair{a:n,b:n}}
 fn run()->Pair<Nat>{duplicate::<Nat>(3)}
 "#,
    )
    .unwrap();
}
#[test]
fn malformed_or_unsatisfied_associated_bounds() {
    let prefix = "trait Source{type Item;fn item()->Self::Item;}struct S{}impl Source for S{type Item=u8;fn item()->u8{1}}";
    for (i, tail) in [
        "fn get<T:Source<Item=u16>>()->u16{T::item()}fn run()->u16{get::<S>()}",
        "fn get<T:Source<Missing=u8>>()->u8{0}",
        "fn get<T:Source<Item=u8,Item=u16>>()->u8{0}",
        "trait Other{type Item;}fn get<T:Source>(x:<T as Other>::Item)->(){ }",
    ]
    .iter()
    .enumerate()
    {
        let e = check(&format!("assoc_bad{i}"), &format!("{prefix}{tail}"))
            .err()
            .expect("reject");
        assert!(e.contains("L0281"), "{e}");
    }
}
#[test]
fn logical_laws_are_explicit_and_checked() {
    check("laws",r#"
 trait Identity{logic fn identity(n:Nat)->Nat;logic fn law(n:Nat)->@(Self::identity(n)==n);}
 struct S{}impl Identity for S{logic fn identity(n:Nat)->Nat{n}logic fn law(n:Nat)->@(Self::identity(n)==n){fold!(Self::identity,prove!(n==n))}}
 logic fn law<T:Identity>(n:Nat)->@(T::identity(n)==n){T::law(n)}
 fn run()->@(S::identity(3)==3){law::<S>(3)}
 "#).unwrap();
    let e=check("no_fold",r#"
 trait Identity{logic fn identity(n:Nat)->Nat;}struct S{}impl Identity for S{logic fn identity(n:Nat)->Nat{n}}
 logic fn bad<T:Identity>(n:Nat)->@(T::identity(n)==n){fold!(T::identity,prove!(n==n))}
 fn run()->@(S::identity(3)==3){bad::<S>(3)}
 "#).err().expect("reject abstract unfolding");
    assert!(e.contains("cannot unfold"), "{e}");
}

#[test]
fn bounds_survive_locals_patterns_and_generic_returns() {
    let prefix = r#"trait Read{fn read(&self)->u8;}struct S{}impl Read for S{fn read(&self)->u8{6}}impl S{fn hidden(&self)->u8{99}}struct Holder<A>{inner:A}fn identity<U>(x:U)->U{x}"#;
    for(i,body)in[
 "fn get<T:Read>(x:T)->u8{let y=identity(x);y.read()}fn run()->u8{get(S{})}",
 "fn get<T:Read>(x:T)->u8{let y:Holder<T>=Holder{inner:x};y.inner.read()}fn run()->u8{get(S{})}",
 "fn get<T:Read>(x:Option<T>)->u8{match x{Option::Some(v)=>v.read(),Option::None=>0}}fn run()->u8{get::<S>(Some(S{}))}",
 "fn get<T:Read>(x:T)->u8{let (v,)=(x,);v.read()}fn run()->u8{get(S{})}",
 ].iter().enumerate(){run(&format!("locals{i}"),&format!("{prefix}{body}"),"6");let bad=body.replace(".read()",".hidden()");let e=check(&format!("locals_bad{i}"),&format!("{prefix}{bad}")).err().expect("must reject hidden method");assert!(e.contains("no declared bound"),"{e}");}
}
#[test]
fn conditional_failures_and_overlapping_families() {
    for(i,code)in[
 "trait Read{fn read(&self)->u8;}struct S{}struct H<T>{x:T}impl<T:Read> Read for H<T>{fn read(&self)->u8{self.x.read()}}fn get<T:Read>(x:T)->u8{x.read()}fn run()->u8{get::<H<S>>(H{x:S{}})}",
 "trait A{}trait B{}struct H<T>{x:T}impl<T:A>A for H<T>{}impl<T:B>A for H<T>{}",
 "trait Read{fn read(&self)->u8;}struct S{}struct H<T>{x:T}impl<T>H<T>{fn get(&self)->u8 where T:Read{self.x.read()}}fn run()->u8{let h:H<S>=H{x:S{}};h.get()}",
 ].iter().enumerate(){assert!(check(&format!("conditional_bad{i}"),code).is_err());}
}
#[test]
#[doc = "spec: 1.31:36"]
fn logical_associated_family_is_checked_without_method_calls() {
    let prefix =
        "trait View{type Item:Logical;}struct H<T>{x:T}impl<T> View for H<T>{type Item=T;}";
    check(
        "assoc_family_good",
        &format!("{prefix}fn make()->H<Nat>{{H{{x:3}}}}"),
    )
    .unwrap();
    let e = check(
        "assoc_family_bad",
        &format!("{prefix}fn make()->H<u8>{{H{{x:3}}}}"),
    )
    .err()
    .expect("associated Logical obligation must be checked");
    assert!(e.contains("Logical") || e.contains("logical"), "{e}");
}
#[test]
#[doc = "spec: 1.31:37"]
fn mutation_and_proof_outputs_execute_once_in_generated_rust() {
    let code = r#"
 trait Step{fn bump(value:&mut u8)->@(value==old!(value).wrapping_add(1));}
 struct S{}impl Step for S{fn bump(value:&mut u8)->@(value==old!(value).wrapping_add(1)){*value=(*value).wrapping_add(1);_}}
 fn change<T:Step>(value:&mut u8)->@(value==old!(value).wrapping_add(1)){T::bump(&mut *value)}
 pub fn run()->u8{let mut n:u8=255;let p=change::<S>(&mut n);n}
 "#;
    run("effects", code, "0");
    let checked = check("effects_rust", code).unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bounds_effects_rust");
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{assert_eq!(run(),0);}}"),
    )
    .unwrap();
    for optimized in [false, true] {
        let mut command = std::process::Command::new("rustc");
        command.args(["--edition=2024", "-Dwarnings"]);
        if optimized {
            command.arg("-O");
        }
        let result = command
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
}

#[test]
fn module_visibility_and_bound_scope_use_real_files() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bounds_modules");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("api.lc"),
        "pub trait Read{fn read(&self)->u8;}pub struct S{}impl Read for S{fn read(&self)->u8{13}}",
    )
    .unwrap();
    std::fs::write(
        dir.join("consumer.lc"),
        "pub fn get<T:crate::api::Read>(x:&T)->u8{x.read()}",
    )
    .unwrap();
    std::fs::write(
        dir.join("export.lc"),
        "mod api;mod consumer;pub fn run()->u8{let s=api::S{};consumer::get(&s)}",
    )
    .unwrap();
    let checked = project::check(&dir.join("export.lc"), &Default::default())
        .unwrap_or_else(|e| panic!("{e}"));
    let m = checked.checked.session.erased();
    let f = checked
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("run"))
        .unwrap()
        .1;
    assert_eq!(
        locus::erased::Interpreter::new(m, 10000)
            .call(f, vec![])
            .unwrap()
            .debug(m),
        "13"
    );
    std::fs::write(
        dir.join("api.lc"),
        "trait Read{fn read(&self)->u8;}pub struct S{}impl Read for S{fn read(&self)->u8{13}}",
    )
    .unwrap();
    let error = project::check(&dir.join("export.lc"), &Default::default())
        .err()
        .expect("private trait cannot be named by a bound")
        .to_string();
    assert!(error.contains("L0503"), "{error}");
}
#[test]
fn generic_exports_do_not_drop_bounds() {
    for (i, code) in [
        "pub trait Read{fn read(&self)->u8;}pub fn get<T:Read>(x:&T)->u8{x.read()}",
        "trait Law{fn law()->@(1==1);}pub fn use_law<T:Law>()->u8{let p=T::law();1}",
        "pub trait A{fn f()->u8 where Self:Logical;}",
        "pub trait A{}struct Holder<T>{value:T}impl<T>A for Holder<T>{}",
    ]
    .iter()
    .enumerate()
    {
        let c = check(&format!("export{i}"), code).unwrap();
        let e = project::rust(c)
            .expect_err("open generic export must not drop its bound")
            .to_string();
        assert!(e.contains("L0504"), "{e}");
    }
}
#[test]
fn associated_constants_and_source_identities_are_preserved() {
    run(
        "constants",
        r#"
 trait Value{const VALUE:u8;}#[derive(Logical)]struct A{}#[derive(Logical)]struct B{}
 impl Value for A{const VALUE:u8=2;}impl Value for B{const VALUE:u8=7;}
 fn get<T:Value>()->u8{T::VALUE}
 fn run()->u8{get::<A>()+get::<B>()}
 "#,
        "9",
    );
}
#[test]
fn body_and_contract_failures_are_not_assumed() {
    for(i,code)in[
 "trait Law{fn law()->@(1==0);}struct S{}impl Law for S{fn law()->@(1==0){_}}fn get<T:Law>()->@(1==0){T::law()}fn run()->@(1==0){get::<S>()}",
 "trait A{fn f()->u8;}trait B{}struct S{}impl A for S{fn f()->u8 where Self:B{1}}",
 "trait A{fn f()->u8;}struct H<T>{v:T}impl<T>A for H<T>{fn f()->u8{false}}fn get<T:A>()->u8{T::f()}fn run()->u8{get::<H<u8>>()}"
 ].iter().enumerate(){assert!(check(&format!("invalid_body{i}"),code).is_err());}
}
#[test]
fn arrays_loop_results_and_logical_closures_keep_bounds() {
    let prefix = "trait Read{fn read(&self)->u8;}#[derive(Logical)]struct S{}impl Read for S{fn read(&self)->u8{3}}impl S{fn hidden(&self)->u8{9}}";
    for (i, body) in [
        "fn get<T:Read>(x:[T;1])->u8{if 0 < x.len() { let v=x[0]; v.read() } else { 0 }}fn run()->u8{get([S{}])}",
        "fn get<T:Read>(x:T)->u8{let v=loop{break x};v.read()}fn run()->u8{get(S{})}",
        "fn get<T:Logical+Read>(x:T)->u8{let f=|v:T|v;let y=f(x);y.read()}fn run()->u8{get(S{})}",
    ]
    .iter()
    .enumerate()
    {
        if i == 2 {
            // Indirect logical calls currently exceed the checking interpreter's
            // coverage (LOC-268). Check typing/erasure and pin that exact gap.
            let c = check("flow_closure", &format!("{prefix}{body}")).unwrap();
            let f = c.checked.functions.iter().find(|(n,_)| n.ends_with("run")).unwrap().1;
            locus::erased::check_module(c.checked.session.erased()).unwrap();
            let out = locus::erased::Interpreter::new(c.checked.session.erased(),10000).call(f,vec![]).unwrap();
            assert_eq!(out.debug(c.checked.session.erased()), "3");
            expected_failure::known_bug("LOC-268", "indirect logical callable in checking interpreter", "a call through a function value",
                locus::exec::CheckInterpreter::new(c.checked.session.program(),10000).with_lending(c.checked.session.lending()).call(f,vec![]).map(|_|()).map_err(|e|format!("{e:?}")));
        } else {
            run(&format!("flow{i}"), &format!("{prefix}{body}"), "3");
        }
        let e = check(
            &format!("flow_bad{i}"),
            &format!("{prefix}{}", body.replace(".read()", ".hidden()")),
        )
        .err()
        .expect("hidden method must be rejected");
        assert!(e.contains("no declared bound"), "{e}");
    }
}
#[test]
fn generic_associated_requirements_cannot_change_in_spec_implementation() {
    let source = r#"
 trait Source{type Item;}struct S{}impl Source for S{type Item=u8;}
 spec type Container<T:Source<Item=u8>>{fn make()->Self;}
 struct Repr<T>{x:T}
 impl<U:Source<Item=u16>> Container<U> for Repr<U>{fn make()->Self{panic!("unused")}}
 "#;
    let e = check("spec_family_mismatch", source)
        .err()
        .expect("bound mismatch");
    assert!(e.contains("same bounds"), "{e}");
}

#[test]
fn requirements_propagate_through_generic_types_and_conditional_families() {
    let prefix = "trait Read{fn read(&self)->u8;}struct S{}impl Read for S{fn read(&self)->u8{6}}struct Holder<T:Read>{value:T}struct Wrapper<T>{value:T}impl<T:Read> Read for Wrapper<T>{fn read(&self)->u8{self.value.read()}}fn read<T:Read>(x:&T)->u8{x.read()}";
    run(
        "entailed_family",
        &format!(
            "{prefix}fn get<T:Read>(x:Wrapper<T>)->u8{{read(&x)}}fn run()->u8{{let w:Wrapper<S>=Wrapper{{value:S{{}}}};get(w)}}"
        ),
        "6",
    );
    for (i, source) in [
        "fn unused<T>(x:Holder<T>)->u8{0}",
        "fn unused<T>(x:&Wrapper<T>)->u8{read(&*x)}",
    ]
    .iter()
    .enumerate()
    {
        let e = check(
            &format!("missing_nested_bound{i}"),
            &format!("{prefix}{source}"),
        )
        .err()
        .expect("missing bound must fail");
        assert!(e.contains("undeclared bound"), "{e}");
    }
}
#[test]
fn logical_associated_classification_is_implied_by_the_interface() {
    check(
        "implied_logical",
        r#"
 trait Source{type Item:Logical;logic fn item()->Self::Item;}
 struct S{}impl Source for S{type Item=Nat;logic fn item()->Nat{1}}
 logic fn identity<T:Logical>(x:T)->T{x}
 logic fn get<T:Source>()->T::Item{identity(T::item())}
 logic fn run()->Nat{get::<S>()}
 "#,
    )
    .unwrap();
}
#[test]
fn mutable_receivers_and_unknown_tuple_prefixes_do_not_hide_capabilities() {
    let prefix = "trait Read{fn read(&mut self)->u8;}struct S{}impl Read for S{fn read(&mut self)->u8{4}}impl S{fn read(&mut self)->u8{8}fn hidden(&mut self)->u8{9}}";
    run(
        "mutable_selection",
        &format!(
            "{prefix}fn get<T:Read>(x:&mut T)->u8{{x.read()}}fn run()->u8{{let mut s=S{{}};get(&mut s)}}"
        ),
        "4",
    );
    for (i, body) in ["(1,x.hidden()).1", "{let ys=[x];ys[0].hidden()}"]
        .iter()
        .enumerate()
    {
        let e = check(
            &format!("unknown_prefix{i}"),
            &format!("{prefix}fn get<T:Read>(x:&mut T)->u8{{{body}}}"),
        )
        .err()
        .expect("hidden capability");
        assert!(e.contains("no declared bound"), "{e}");
    }
}

#[test]
fn constructed_containers_and_inherent_results_preserve_abstract_types() {
    let prefix = "trait Read{fn read(&self)->u8;}#[derive(Logical)]struct S{}impl Read for S{fn read(&self)->u8{5}}impl S{fn hidden(&self)->u8{9}}struct Holder<U>{inner:U}impl<U> Holder<U>{fn inner(self)->U{self.inner}}";
    for (i, body) in [
        "let wrapped=Option::<T>::Some(x);match wrapped {Option::Some(v)=>v.read(),Option::None=>0}",
        "let wrapped:Holder<T>=Holder{inner:x};let v=wrapped.inner();v.read()",
    ]
    .iter()
    .enumerate()
    {
        let source = format!("{prefix}fn get<T:Read>(x:T)->u8{{{body}}}fn run()->u8{{get(S{{}})}}");
        run(&format!("constructed{i}"), &source, "5");
        let e = check(
            &format!("constructed_hidden{i}"),
            &source.replace("v.read()", "v.hidden()"),
        )
        .err()
        .expect("hidden member must fail");
        assert!(e.contains("no declared bound"), "{e}");
    }
}

#[test]
#[doc = "spec: 1.18:3"]
fn borrowing_erased_locals_retains_only_marker_storage() {
    let code = r#"
 #[derive(Logical)]struct S{}
 trait Read{fn read(&self,n:&mut u8)->u8;}
 impl Read for S{fn read(&self,n:&mut u8)->u8{*n=(*n).wrapping_add(1);*n}}
 fn get<T:Read>(x:T,n:&mut u8)->u8{x.read(&mut *n)}
 pub fn run()->u8{let x=S{};let mut n:u8=6;get(x,&mut n)}
 "#;
    run("borrowed_erased", code, "7");
    let checked = check("borrowed_erased_rust", code).unwrap();
    locus::erased::check_module(checked.checked.session.erased()).unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bounds_borrowed_erased_rust");
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main(){{assert_eq!(run(),7);}}"),
    )
    .unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings"])
        .arg(dir.join("main.rs"))
        .arg("-o")
        .arg(dir.join("app"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{rust}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::process::Command::new(dir.join("app"))
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn reborrows_infer_the_referent_without_inventing_reference_implementations() {
    let code = r#"
 trait Read{fn read(&self)->u8;}struct S{}impl Read for S{fn read(&self)->u8{6}}
 struct Wrapper<T>{value:T}impl<T:Read> Read for Wrapper<T>{fn read(&self)->u8{self.value.read()}}
 fn read<T:Read>(x:&T)->u8{x.read()}
 fn get<T:Read>(x:&Wrapper<T>)->u8{read(&*x)}
 fn run()->u8{let w:Wrapper<S>=Wrapper{value:S{}};get(&w)}
 "#;
    run("reborrow", code, "6");
    let e=check("reference_not_impl","trait Read{}struct S{}impl Read for S{}fn use_it<T:Read>()->u8{1}fn run()->u8{use_it::<&S>()}").err().expect("reference is a different type");
    assert!(e.contains("does not satisfy trait bound"), "{e}");
}

#[test]
fn recursive_type_requirements_fail_without_overflowing_the_compiler() {
    let e = check("cyclic_header", "trait A{}struct W<T> where W<T>:A {v:T}")
        .err()
        .expect("cyclic requirement");
    assert!(e.contains("MAX_GENERIC_TYPE_DEPTH"), "{e}");
}
