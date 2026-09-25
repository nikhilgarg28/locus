//! References are transparent at logical call boundaries, never at runtime ones.
use locus::{
    elab::{Elaborated, elaborate},
    parser::parse,
    source::SourceMap,
};
#[path = "common/compiled.rs"]
mod compiled;
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("observation.lc", text);
    let source = sources.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{text}\n{:#?}", parsed.diagnostics);
    elaborate(source, &parsed.program)
}
fn accepted(text: &str) -> Elaborated {
    let result = check(text);
    assert!(result.is_success(), "{text}\n{:#?}", result.diagnostics);
    locus::erased::check_module(result.session.erased()).unwrap();
    result
}
fn rejected(text: &str) {
    let result = check(text);
    assert!(!result.is_success(), "accepted {text}");
    assert!(
        result.diagnostics.iter().all(|d| d.code != "L0300"),
        "{:#?}",
        result.diagnostics
    );
}
fn runs(text: &str, expected: &str) {
    let checked = accepted(text);
    let id = checked.function("run").unwrap();
    let original = locus::exec::CheckInterpreter::new(checked.session.program(), 10000)
        .with_lending(checked.session.lending())
        .call(id, vec![])
        .unwrap();
    let erased = locus::erased::Interpreter::new(checked.session.erased(), 10000)
        .call(id, vec![])
        .unwrap();
    assert_eq!(original, erased);
    assert_eq!(erased.debug(checked.session.erased()), expected);
    let rust = compiled::harness(&[compiled::Unit {
        module: "observation".into(),
        rust: locus::erased::print_module(checked.session.erased()),
        calls: vec!["observation::run()".into()],
    }]);
    for mode in compiled::Overflow::ALL {
        let binary =
            compiled::compile(std::thread::current().name().unwrap(), &rust, mode).unwrap();
        assert_eq!(
            compiled::observe(&binary, 1, std::time::Duration::from_secs(10)),
            vec![compiled::Answered::Value(expected.into())]
        );
    }
}
#[test]
#[doc = "spec: 1.95:1"]
fn declarations_and_arguments_accept_every_shared_reference_depth() {
    for ty in ["u8", "&u8", "&&u8", "& & &u8", "(&'a (&'b u8))"] {
        for argument in ["x", "&x", "&&x", "r", "&r", "rr", "&&rr"] {
            accepted(&format!("logic fn P<'a,'b>(x:{ty})->Prop{{prop!(x==7)}}
                fn run()->(){{let x:u8=7;let r=&x;let rr=&r;let p:@P({argument})=fold!(P,prove!(x==7));let q:@P(x)=p;}}"));
        }
    }
}
#[test]
#[doc = "spec: 1.95:1"]
fn physical_observations_do_not_move_noncopy_values_or_keep_loans() {
    runs("struct Item{n:u8} logic fn P(x:Item)->Prop{prop!(model!(x.n)>0)}
        fn consume(x:Item)->u8{x.n}
        pub fn run()->u8{let mut x=Item{n:7};let r=&x;let p:@P(r)=fold!(P,prove!(model!(x.n)>0));x=Item{n:8};let old:@(7>0)=unfold!(P,p);consume(x)}", "8");
}
#[test]
#[doc = "spec: 1.95:2"]
fn reference_normalization_preserves_read_permissions_and_versions() {
    let prefix = "logic fn P(x:&&u8)->Prop{prop!(x>0)}";
    for tail in [
        "fn bad()->(){let mut x:u8=7;let r=&x;let rr=&r;x=0;let p=P(rr);}",
        "fn bad()->(){let mut x:u8=7;let p:@P(x)=fold!(P,prove!(x>0));x=0;let q:@P(x)=p;}",
        "fn bad()->(){let mut x:u8=7;let mut p:@P(&x)=fold!(P,prove!(x>0));x=0;let q=p;}",
    ] {
        rejected(&format!("{prefix}{tail}"));
    }
    rejected(
        "struct Item{n:u8} fn consume(x:Item)->u8{x.n} logic fn P(x:Item)->Prop{prop!(model!(x.n)>0)} fn bad()->(){let x=Item{n:7};let n=consume(x);let p=P(x);}",
    );
    runs(
        "logic fn P(x:&&u8)->Prop{prop!(x>0)} fn work(x:&mut u8,p:@(x>0))->(){let observed:@P(x)=fold!(P,p);x=0;} pub fn run()->u8{let mut x:u8=7;work(&mut x,prove!(x>0));x}",
        "0",
    );
}
#[test]
#[doc = "spec: 1.95:3, 3.8:2"]
fn canonical_models_are_selected_after_reference_normalization() {
    for ty in ["Point", "&Point", "&&Point"] {
        accepted(&format!("struct Point{{x:u8}} #[derive(Logical)] struct Position{{x:Nat}}
            impl Model for Point{{type Logic=Position;logic fn model(p:{ty})->Position{{Position{{x:model!(p.x)}}}}}}
            logic fn P(p:&&Position)->Prop{{prop!(p.x==7)}}
            fn run()->(){{let point=Point{{x:7}};let r=&point;let rr=&r;let p:@P(rr)=fold!(P,prove!(point.x==7));let q:@P(model!(point))=p;}}"));
    }
    accepted(
        "logic fn P(n:&&Nat)->Prop{prop!(n==7)} fn run()->(){let x:u8=7;let r=&x;let p:@P(r)=fold!(P,prove!(x==7));}",
    );
}
#[test]
fn generic_inference_normalizes_only_outer_references() {
    for parameter in ["T", "&T", "&&T"] {
        accepted(&format!("logic fn P<T>(x:{parameter})->Prop{{prop!(0==0)}}
            fn run()->(){{let x:u8=7;let r=&x;let rr=&r;let p:@P(rr)=fold!(P::<u8>,prove!(0==0));let q:@P::<u8>(x)=p;}}"));
    }
    rejected("logic fn P(x:u8)->Prop{prop!(true)} fn bad()->(){let b=Box::new(7u8);let p=P(b);}");
    rejected(
        "logic fn P(x:Option<u8>)->Prop{prop!(true)} fn bad()->(){let x:u8=7;let p=P(Some(&x));}",
    );
}
#[test]
fn logical_receivers_and_associated_calls_are_observations() {
    runs("struct Item{n:u8} impl Item{logic fn value(&self)->Nat{model!(self.n)} logic fn positive(self)->Prop{prop!(model!(self.n)>0)}}
        fn make(n:u8)->Item{Item{n}}
        pub fn run()->u8{let x=Item{n:7};let a=x.value();let b=Item::value(&&x);let p:@Item::positive(&x)=fold!(Item::positive,prove!(model!(x.n)>0));let c=make(9).value();x.n}","7");
}
#[test]
#[doc = "spec: 1.95:2"]
fn eager_runtime_effects_and_moves_survive_erased_calls() {
    runs(
        "logic fn P(x:&&u8)->Prop{prop!(x>0)} fn bump(x:&mut u8)->u8{x=x.wrapping_add(1);x}
        pub fn run()->u8{let mut x:u8=0;let p=P(bump(&mut x));x}",
        "1",
    );
    rejected(
        "struct Item{n:u8} fn consume(x:Item)->u8{x.n} logic fn P(x:&&u8)->Prop{prop!(true)} fn bad()->u8{let x=Item{n:7};let p=P(consume(x));x.n}",
    );
}
#[test]
fn runtime_calling_conventions_and_logical_purity_do_not_change() {
    rejected("fn take(x:&u8)->u8{x} fn bad(x:u8)->u8{take(x)}");
    rejected("fn take(x:u8)->u8{x} fn bad(x:u8)->u8{take(&x)}");
    rejected("logic fn bad(x:&mut u8)->Prop{prop!(true)}");
    rejected("logic fn bad(x:&&mut u8)->Prop{prop!(true)}");
    rejected("fn read()->u8{7} logic fn P(x:u8)->Prop{prop!(x>0)} logic fn bad()->Prop{P(read())}");
}
#[test]
fn structural_model_conversion_evaluates_a_runtime_argument_once() {
    runs(
        "#[derive(Model)] struct Pair{a:u8,b:u8}
        fn make(n:&mut u8)->Pair{n=n.wrapping_add(1);Pair{a:n,b:n}}
        logic fn P(pair:&PairModel)->Prop{prop!(pair.a==pair.b)}
        pub fn run()->u8{let mut n:u8=0;let claim=P(make(&mut n));n}",
        "1",
    );
}
#[test]
fn slice_observation_accepts_owned_arrays_vectors_and_nested_references() {
    for ty in ["[u32]", "&[u32]", "&&[u32]"] {
        accepted(&format!("logic fn P(items:{ty})->Prop{{prop!(items.len()==2)}}
            fn run()->(){{let xs:[u32;2]=[1,3];let r=&xs;let rr=&r;let p:@P(rr)=fold!(P,prove!(xs.len()==2));let q:@P(xs)=p;let ys:Vec<u32>=Vec::from([2,4]);let y=P(ys);}}"));
    }
}
#[test]
fn logical_observation_does_not_dereference_boxes_or_container_fields() {
    accepted(
        "logic fn P(x:Box<u8>)->Prop{prop!(model!(*x)==7)} fn run()->(){let b=Box::new(7u8);let p:@P(&b)=fold!(P,prove!(model!(*b)==7));}",
    );
    rejected("logic fn P(x:u8)->Prop{prop!(x>0)} fn bad()->(){let xs:[u8;1]=[7];let p=P(xs);}");
}
#[test]
fn logical_callable_types_closures_and_recursion_normalize_observations() {
    accepted(
        "logic fn apply(f:logic Fn(n:&&Int)->Int,n:&Int)->Int{f(&&n)}
        logic fn id(n:&&Int)->Int{n}
        logic fn sample()->@(apply(|n:Int|id(n),7)==7){prove!(apply(|n:Int|id(n),7)==7)}
        logic fn closure()->logic Fn(n:Int)->Int{|n:&&Int|n}",
    );
    accepted("#[derive(Logical)] enum Peano{Zero,Succ(Peano)}
        logic fn size(n:&&Peano)->Int{match n{Peano::Zero=>0,Peano::Succ(p)=>1+size(&p)}}
        logic fn one()->@(size(Peano::Succ(Peano::Zero))==1){prove!(size(Peano::Succ(Peano::Zero))==1)}");
}
#[test]
fn logical_parameters_keep_model_field_selection_distinct_from_physical_paths() {
    accepted("struct Point{x:u8} #[derive(Logical)] struct Position{abstract_x:Nat}
        impl Model for Point{type Logic=Position;logic fn model(p:Point)->Position{Position{abstract_x:model!(p.x)}}}
        logic fn Q(n:Nat)->Prop{prop!(n==7)}
        fn proof(p:Point,h:@(p.abstract_x==7))->@Q(p.abstract_x){fold!(Q,h)}");
    rejected("struct Point{x:u8} #[derive(Logical)] struct Position{abstract_x:Nat}
        impl Model for Point{type Logic=Position;logic fn model(p:Point)->Position{Position{abstract_x:model!(p.x)}}}
        logic fn Q(n:Nat)->Prop{prop!(n==7)} fn wrong(p:Point)->Prop{logic{Q(p.x)}}");
}
#[test]
fn physical_and_logical_boolean_parameters_preserve_modes() {
    accepted("logic fn Physical(b:&bool)->Prop{prop!(b==b)} logic fn Logical(b:&&Bool)->Prop{prop!(b==b)}
        fn physical(b:bool,p:@Physical(b))->@Physical(&&b){p}
        fn logical(b:bool,p:@Logical(b))->@Logical(&&b){p}
        fn literals()->@Physical(true){fold!(Physical,prove!(true==true))}");
    rejected("logic fn Physical(b:bool)->Prop{prop!(b==b)} fn bad(b:Bool)->Prop{Physical(b)}");
}
#[test]
fn structural_observations_keep_argument_order_and_generated_binding_hygiene() {
    runs("#[derive(Model)] struct Pair{a:u8,b:u8}
        fn make(n:&mut u8)->Pair{n=n.wrapping_add(1);Pair{a:n,b:n}}
        logic fn P(a:PairModel,b:&&PairModel)->Prop{prop!(a.a<b.b)}
        pub fn run()->u8{let __locus_observation:u8=99;let mut n:u8=0;let p=P(make(&mut n),make(&mut n));n.wrapping_add(__locus_observation)}","101");
}
