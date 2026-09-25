//! Logical indices preserve scoped evidence through nominal generic containers.
use locus::{
    elab::{Elaborated, elaborate},
    parser::parse,
    source::SourceMap,
};
fn check(text: &str) -> Elaborated {
    let mut sources = SourceMap::default();
    let id = sources.add("scoped.lc", text);
    let source = sources.get(id);
    let parsed = parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    elaborate(source, &parsed.program)
}
fn accepted(text: &str) -> Elaborated {
    let result = check(text);
    assert!(result.is_success(), "{:#?}", result.diagnostics);
    assert_eq!(locus::erased::check_module(result.session.erased()), Ok(()));
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
#[test]
#[doc = "spec: 1.27:13"]
fn optional_claim_crosses_calls_results_and_matches() {
    accepted(include_str!("../examples/optional_evidence.lc"));
}
#[test]
fn arbitrary_nominal_containers_nesting_and_field_projection() {
    accepted("struct Holder<T> { value: T }
        enum Either<L, R> { Left { value: L }, Right(R) }
        fn need(n:u8, p:@(n>0)) -> () {}
        fn take(n:u8, xs: Either<Holder<@(n>0)>, Option<@(n==0)>>) -> () {
            match xs { Either::Left { value } => need(n, value.value),
                Either::Right(opt) => { match opt { Option::Some(p) => { let q:@(n==0)=p; }, Option::None => {} } } }
        }
        fn make(n:u8, p:@(n>0)) -> Either<Holder<@(n>0)>, Option<@(n==0)>> {
            Either::Left { value: Holder { value: p } }
        }");
}
#[test]
fn distinct_snapshot_cannot_be_substituted() {
    rejected("fn wrong(n:u8,m:u8,p:Option<@(n>0)>)->Option<@(m>0)>{p}");
    rejected("fn wrong(n:u8,p:@(n>0))->Option<@(n==0)>{Some(p)}");
    rejected("fn wrong(n:u8,p:Option<@(n>0)>)->@(n==0){match p {Some(h)=>h,None=>prove!(n==0)}}");
    rejected(
        "fn need(n:u8,p:Option<@(n>0)>)->(){} fn wrong(n:u8,p:Option<@(n>0)>)->(){let n:u8=0;need(n,p)}",
    );
}
#[test]
fn mutable_snapshot_is_not_silently_retargeted() {
    rejected(
        "fn need(n:u8,p:Option<@(n>0)>)->(){} fn wrong()->(){let mut n:u8=1;let p:Option<@(n>0)>=Some(prove!(n>0)); n=0; need(n,p);}",
    );
    rejected(
        "fn wrong()->(){let mut n:u8=1;let mut p:Option<@(n>0)>=Some(prove!(n>0)); n=0; let q=p;}",
    );
}
#[test]
fn explicit_arguments_do_not_get_overridden_by_context() {
    rejected("fn wrong(n:u8,p:@(n>0))->Option<@(n==0)>{Option::<@(n>0)>::Some(p)}");
    rejected("fn wrong(n:u8)->Option<@(n==0)>{Option::<@(n>0)>::None}");
}
#[test]
fn scope_exit_and_logical_control_are_rejected() {
    rejected(
        "fn read()->u8{3} fn bad()->(){let p={let hidden=read();let p:Option<@(hidden==hidden)>=Some(prove!(hidden==hidden));p};}",
    );
    rejected(
        "fn bad(n:u8,p:Option<@(n>0)>)->u8{match p {Option::Some(h)=>match h {_=>1},Option::None=>0}}",
    );
}
#[test]
fn stored_proof_claim_uses_its_own_snapshot() {
    accepted(
        "fn need(n:u8,p:Option<@(n>0)>)->(){} fn good(n:u8,p:Option<@(n>0)>)->Option<@(n>0)>{let n:u8=0;p}",
    );
    accepted("struct Packet { n:u8, p:Option<@(n>0)> }
      fn read(p:Packet)->(){ match p.p {Option::Some(h)=>{let q:@(model!(p.n)>0)=h;},Option::None=>{}} }");
}
#[test]
fn sibling_evidence_prevents_partial_mutation_and_public_forgery() {
    rejected("struct Packet { n:u8, p:Option<@(n>0)> } fn bad(p:&mut Packet)->(){p.n=0;}");
    rejected("pub struct Packet { pub n:u8, p:Option<@(n>0)> }");
    rejected("pub fn bad(n:u8,p:Option<@(n>0)>)->(){}");
    rejected("struct Holder<T>{value:T} pub fn bad(n:u8,p:Holder<@(n>0)>)->(){}");
}
#[test]
fn generated_capture_names_are_hygienic() {
    accepted(
        "struct Holder<T>{__locus_claim_0:Prop,value:T}
        fn make(n:u8,p:@(n>0))->Holder<@(n>0)>{Holder{__locus_claim_0:prop!(false),value:p}}",
    );
}
#[test]
fn ordinary_effects_inside_proof_payloads_are_preserved() {
    let checked=accepted("fn bump(n:&mut u8)->@True{n=n.wrapping_add(1);True::Intro}
        pub fn run()->u8{let mut n:u8=0;let p:Option<@True>=Some(bump(&mut n));match p {Option::Some(h)=>n,Option::None=>0}}");
    let id = checked.function("run").unwrap();
    let erased = locus::erased::Interpreter::new(checked.session.erased(), 10000)
        .call(id, vec![])
        .unwrap();
    let logical = locus::exec::CheckInterpreter::new(checked.session.program(), 10000)
        .with_lending(checked.session.lending())
        .call(id, vec![])
        .unwrap();
    assert_eq!(erased, logical);
    assert_eq!(erased.debug(checked.session.erased()), "1");
    let source = compiled::harness(&[compiled::Unit {
        module: "effects".into(),
        rust: locus::erased::print_module(checked.session.erased()),
        calls: vec!["effects::run()".into()],
    }]);
    for mode in compiled::Overflow::ALL {
        let binary = compiled::compile("optional_evidence_effects", &source, mode).unwrap();
        assert_eq!(
            compiled::observe(&binary, 1, std::time::Duration::from_secs(10)),
            vec![compiled::Answered::Value("1".into())]
        );
    }
}
#[path = "common/compiled.rs"]
mod compiled;
#[test]
#[doc = "spec: 1.27:13, 1.18:4"]
fn search_agrees_across_checked_ir_erasure_and_both_rust_builds() {
    use locus::erased::{EType, Outcome, Value};
    use locus::kernel::MachineInt;
    let checked = accepted(include_str!("../examples/optional_search.lc"));
    let module = checked.session.erased();
    let f = module.fns.iter().find(|f| f.name == "search").unwrap();
    let EType::Enum(evidence) = f.params[2].2 else {
        panic!()
    };
    let EType::Enum(answer) = f.result else {
        panic!()
    };
    let evidence_name = &module.enums.iter().find(|e| e.id == evidence).unwrap().name;
    let answer_name = &module.enums.iter().find(|e| e.id == answer).unwrap().name;
    let mut calls = Vec::new();
    let mut expected = Vec::new();
    let locus::typed::FnRef::Math(prefix) = checked.function("ordered_prefix").unwrap() else {
        panic!()
    };
    let definitions = std::rc::Rc::new(checked.session.program().definitions().clone());
    let signature = definitions.signature(prefix).unwrap();
    let alphabet = [0u32, 1, 2, u32::MAX];
    for len in 0..=4u32 {
        for mut code in 0..4usize.pow(len) {
            let mut items = Vec::new();
            for _ in 0..len {
                items.push(alphabet[code % 4]);
                code /= 4;
            }
            let sorted = items.windows(2).all(|w| w[0] <= w[1]);
            // Check the logical definition against an independent ordering
            // oracle as well as testing the runtime algorithms. The harness
            // supplies only the true range premise count == input length;
            // it supplies no assumption of sortedness.
            use locus::kernel::{
                BufferOp, Context, HypRef, Proof, Term, Type, infer_proof, telescope_entry,
            };
            let mut arguments = vec![
                Term::Buffer {
                    op: BufferOp::Literal,
                    element: Type::machine(MachineInt::U32),
                    arguments: items
                        .iter()
                        .map(|n| Term::machine_int(MachineInt::U32, *n as i128))
                        .collect(),
                },
                Term::Int((len as i128).into()),
            ];
            let Type::Proof(range) = telescope_entry(&signature, 2, &arguments).unwrap() else {
                panic!()
            };
            let mut context = Context::with_definitions(std::rc::Rc::clone(&definitions));
            let bound = context.assume(*range).unwrap();
            arguments.push(Term::proof(Proof::Hyp(HypRef::Free(bound))));
            let computation = Term::call(Term::Fn(prefix), arguments);
            let Term::Eq(_, _, result) =
                infer_proof(&mut context, &Proof::Evaluate(computation)).unwrap()
            else {
                panic!()
            };
            assert_eq!(
                *result,
                Term::Bool(sorted),
                "logical sortedness disagrees for {items:?}"
            );
            for key in [0u32, 1, 2, 3, u32::MAX] {
                for known in [false, true] {
                    if known && !sorted {
                        continue;
                    }
                    // The test driver supplies erased evidence only when its
                    // independent finite model establishes the precondition.
                    let args = vec![
                        Value::Buffer(
                            items
                                .iter()
                                .map(|n| Value::Int(MachineInt::U32, *n as i128))
                                .collect(),
                        ),
                        Value::Int(MachineInt::U32, key as i128),
                        Value::Variant(
                            evidence,
                            usize::from(known),
                            if known { vec![Value::Proved] } else { vec![] },
                        ),
                    ];
                    let original =
                        locus::exec::CheckInterpreter::new(checked.session.program(), 100_000)
                            .call(f.reference, args.clone())
                            .unwrap();
                    let erased = locus::erased::Interpreter::new(module, 100_000)
                        .call(f.reference, args)
                        .unwrap();
                    assert_eq!(original, erased, "{items:?}, {key}, {known}");
                    let Outcome::Value(Value::Variant(id, tag, ref payload)) = erased else {
                        panic!("{erased:?}")
                    };
                    assert_eq!(id, answer);
                    let (found, index) = if tag == 0 {
                        (false, 0)
                    } else {
                        let [Value::Int(MachineInt::U64, index)] = payload.as_slice() else {
                            panic!()
                        };
                        (true, *index as usize)
                    };
                    assert_eq!(found, items.contains(&key));
                    if found {
                        assert_eq!(items.get(index), Some(&key));
                    }
                    calls.push(format!("searches::observe(&{items:?}, {key}, {known})"));
                    expected.push(compiled::Answered::Value(format!("({found}, {index})")));
                }
            }
        }
    }
    let mut rust = locus::erased::print_module(module);
    assert!(!rust.contains("fn Sorted"));
    assert!(!rust.contains("fn ordered_prefix"));
    assert!(rust.contains("Some(Erased)"));
    rust.push_str(&format!("\npub fn observe(items: &[u32], key:u32, known:bool)->(bool,u64) {{
        let evidence=if known {{{evidence_name}::Some(Erased)}}else{{{evidence_name}::None}};
        match search(items,key,evidence) {{{answer_name}::None=>(false,0),{answer_name}::Some(i)=>(true,i)}}
    }}"));
    let source = compiled::harness(&[compiled::Unit {
        module: "searches".into(),
        rust,
        calls,
    }]);
    for mode in compiled::Overflow::ALL {
        let binary = compiled::compile("optional_search", &source, mode).unwrap();
        assert_eq!(
            compiled::observe(&binary, expected.len(), std::time::Duration::from_secs(30)),
            expected
        );
    }
}

#[test]
fn effectful_constructor_arguments_capture_the_normal_return_snapshot() {
    accepted(
        "fn bump(n:&mut u8)->@(n>0){n=1;prove!(n>0)} fn wrap(n:&mut u8)->Option<@(n>0)>{Some(bump(&mut n))}",
    );
    accepted(
        "struct Hold<T>{p:T} fn bump(n:&mut u8)->@(n>0){n=1;prove!(n>0)} fn wrap(n:&mut u8)->Hold<@(n>0)>{Hold{p:bump(&mut n)}}",
    );
}

#[test]
fn unsorted_input_and_mutated_list_cannot_supply_sorted_evidence() {
    let example = include_str!("../examples/optional_search.lc");
    let false_claim = example.replace("[1, 3, 5]", "[3, 1, 5]");
    rejected(&false_claim);
    let stale = format!(
        "{example} fn stale(xs:&mut [u32], p:Option<@Sorted(&xs)>)->Option<u64>{{if xs.len()>0{{xs[0]=0;search(&xs,0,p)}}else{{None}}}}"
    );
    rejected(&stale);
}
#[test]
fn branch_join_and_tracked_container_require_fresh_evidence() {
    accepted(
        "fn need(n:u8,p:Option<@(n>0)>)->(){} fn good()->(){let mut n:u8=1;let mut p:Option<@(n>0)>=Some(prove!(n>0));n=2;p=Some(prove!(n>0));need(n,p);}",
    );
    rejected(
        "fn bad(flag:bool)->(){let mut n:u8=1;let mut p:Option<@(n>0)>=Some(prove!(n>0));if flag{n=0;}else{}let q=p;}",
    );
    accepted("fn choose(n:u8,p:@(n>0),yes:bool)->Option<@(n>0)>{if yes{Some(p)}else{None}}");
}
#[test]
fn closed_function_arguments_and_logical_aggregate_families_stay_checked() {
    accepted(
        "fn id<T>(value:T)->T{value} fn run()->Option<@True>{id::<Option<@True>>(Some(True::Intro))}",
    );
    accepted(
        "#[derive(Logical)] enum Evidence<T:Logical>{Held(T)} logic fn carry(n:Int,p:@(n==n))->Evidence<@(n==n)>{Evidence::Held(p)}",
    );
    rejected("fn id<T>(value:T)->T{value} fn bad(n:u8,p:@(n>0))->@(n>0){id::<@(n>0)>(p)}");
}

#[test]
fn generic_templates_can_nest_other_generic_containers_of_their_parameter() {
    accepted("struct Carrier<T>{value:Option<T>}
        fn make(n:u8,p:@(n>0))->Carrier<@(n>0)>{Carrier{value:Some(p)}}
        fn use_it(n:u8,c:Carrier<@(n>0)>)->(){match c.value{Option::Some(h)=>{let p:@(n>0)=h;},Option::None=>{}}}");
}

#[test]
#[doc = "spec: 1.18:4"]
fn logical_families_require_logical_match() {
    rejected(
        "#[derive(Logical)] enum Evidence<T:Logical>{No,Yes(T)}
        fn leak(n:u8,p:Evidence<@(n>0)>)->u8 {
            match p {Evidence::No=>0,Evidence::Yes(h)=>1}
        }",
    );
    accepted(
        "#[derive(Logical)] enum Evidence<T:Logical>{No,Yes(T)}
        logic fn inspect(n:u8,p:Evidence<@(n>0)>)->Bool {
            logic {match p {Evidence::No=>false,Evidence::Yes(h)=>true}}
        }",
    );
}
