use locus::project;
use std::path::PathBuf;

fn check(tag: &str, code: &str) -> Result<project::Checked, String> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dyn_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("export.lc");
    std::fs::write(&path, code).unwrap();
    project::check(&path, &Default::default()).map_err(|e| e.to_string())
}

#[test]
#[doc = "spec: 1.31:50"]
#[doc = "spec: 1.31:52"]
fn borrowed_dispatch() {
    let checked = check(
        "basic",
        r#"
trait Read { fn read(&self) -> u8; }
struct Counter { n: u8 }
impl Read for Counter { fn read(&self) -> u8 { self.n } }
fn read(x: &dyn Read) -> u8 { x.read() }
pub fn answer() -> u8 { let c = Counter { n: 42 }; read(&c) }
"#,
    )
    .unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("dyn_basic");
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main() {{ assert_eq!(answer(),42); }}"),
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

fn run(tag: &str, code: &str, expected: &str) {
    run_checked(tag, check(tag, code).unwrap(), expected);
}

fn run_checked(tag: &str, checked: project::Checked, expected: &str) {
    let session = &checked.checked.session;
    let function = checked
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("_answer"))
        .unwrap()
        .1;
    for overflow in locus::erased::Overflow::ALL {
        let a = locus::exec::CheckInterpreter::new(session.program(), 10000)
            .with_lending(session.lending())
            .with_overflow(overflow)
            .call(function, vec![])
            .unwrap();
        let b = locus::erased::Interpreter::new(session.erased(), 10000)
            .with_overflow(overflow)
            .call(function, vec![])
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a.debug(session.erased()), expected);
    }
    let rust = project::rust(checked).unwrap();
    assert!(rust.contains("dyn LocusMDyn"));
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dyn_{tag}"));
    std::fs::write(
        dir.join("main.rs"),
        format!("{rust}\nfn main() {{ assert_eq!(answer(),{expected}); }}"),
    )
    .unwrap();
    for checks in ["yes", "no"] {
        let output = std::process::Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "-C"])
            .arg(format!("overflow-checks={checks}"))
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
}

const TWO: &str = r#"
trait Read { fn read(&self) -> u8; fn plus(&self, n: u8) -> u8 { self.read() + n } }
struct A { n:u8 }
struct B { n:u8 }
impl Read for A {fn read(&self)->u8 {self.n}}
impl Read for B {fn read(&self)->u8 {self.n+1}}
impl A {fn read(&self)->u8 {99}}
fn consume(r:&dyn Read)->u8 {r.plus(2)}
fn choose(flag:bool)->u8 {
    let a=A{n:10}; let b=B{n:20};
    let object: &dyn Read = if flag { &a } else { &b };
    consume(object) + object.read()
}
"#;

#[test]
#[doc = "spec: 1.31:53"]
fn runtime_selection_defaults_and_inherent_collision() {
    run(
        "selection",
        &format!("{TWO} pub fn answer()->(u8,u8){{(choose(true),choose(false))}}"),
        "(22, 44)",
    );
}

#[test]
fn casts_forwarding_and_aliased_traits() {
    run(
        "casts",
        r#"
mod contract {pub trait Read {fn read(&self)->u8;}}
use contract::Read as Reader;
struct S(u8);
impl Reader for S {fn read(&self)->u8{self.0}}
fn consume(r:&dyn Reader)->u8{r.read()}
fn forward(r:&dyn contract::Read)->u8{consume(r)}
pub fn answer()->u8{let s=S(37);let r=&s as &dyn Reader;forward(r)}
"#,
        "37",
    );
}

#[test]
#[doc = "spec: 1.31:51"]
fn fixed_associated_types_and_multiple_slots() {
    run(
        "associated",
        r#"
trait Source {type Item;fn read(&self)->Self::Item;fn test(&self,n:Self::Item)->bool;}
struct S{}
impl Source for S {type Item=u8;fn read(&self)->u8{9}fn test(&self,n:u8)->bool{n==9}}
fn consume(r:&dyn Source<Item=u8>)->(u8,bool){(r.read(),r.test(9))}
pub fn answer()->(u8,bool){let s=S{};consume(&s)}
"#,
        "(9, true)",
    );
}

#[test]
#[doc = "spec: 1.31:51"]
#[doc = "spec: 1.31:55"]
fn incompatible_interfaces_are_explained() {
    for (i, source) in [
        "trait A{fn f()->u8;}fn use_it(r:&dyn A){}",
        "trait A{fn f(&mut self);}fn use_it(r:&dyn A){}",
        "trait A{fn f(self);}fn use_it(r:&dyn A){}",
        "trait A{fn f(&self)->Self;}fn use_it(r:&dyn A){}",
        "trait A{fn f(&self,x:Self);}fn use_it(r:&dyn A){}",
        "trait A{fn f(&self)->@(1==1);}fn use_it(r:&dyn A){}",
        "trait A{fn f(&self,p:@(1==1));}fn use_it(r:&dyn A){}",
        "trait A{logic fn f()->Int;}fn use_it(r:&dyn A){}",
        "trait A{const N:u8;}fn use_it(r:&dyn A){}",
        "trait A{type X:Logical;}fn use_it(r:&dyn A<X=Nat>){}",
        "trait A{type X;}fn use_it(r:&dyn A){}",
        "trait A{}fn use_it(r:&dyn A<X=u8>){}",
        "trait A{type X;}fn use_it(r:&dyn A<X=u8,X=u8>){}",
        "trait A{#[no_panic] fn f(&self)->u8;}fn use_it(r:&dyn A){}",
        "trait A{}fn use_it(r:dyn A){}",
        "trait A{}fn use_it(r:&mut dyn A){}",
        "trait A{}fn use_it(r:Box<dyn A>){}",
    ]
    .iter()
    .enumerate()
    {
        let error = check(&format!("incompatible_{i}"), source)
            .err()
            .expect(source);
        assert!(error.contains("L0518"), "{source}\n{error}");
    }
}

#[test]
fn bad_coercions_and_associated_mismatch() {
    for (i,source) in [
        "trait A{}struct S{}fn f(r:&dyn A){}fn g(){let s=S{};f(&s)}",
        "trait A{}struct S{}impl A for S{}fn f(r:&dyn A){}fn g(){let s=S{};f(s)}",
        "trait A{type X;}struct S{}impl A for S{type X=u16;}fn f(r:&dyn A<X=u8>){}fn g(){let s=S{};f(&s)}",
    ].iter().enumerate() {
        let error=check(&format!("coercion_bad_{i}"),source).err().expect(source);
        assert!(error.contains("L0518") || error.contains("L0272"),"{source}\n{error}");
    }
}

#[test]
#[doc = "spec: 1.31:54"]
fn borrowed_objects_keep_storage_alive() {
    for (i, tail) in [
        "let r:&dyn Read=&s; s.n=5; r.read()",
        "let r:&dyn Read=&s; let moved=s; r.read()",
        "let r:&dyn Read={let local=S{n:1};&local};r.read()",
    ]
    .iter()
    .enumerate()
    {
        let source = format!(
            "trait Read{{fn read(&self)->u8;}}struct S{{n:u8}}impl Read for S{{fn read(&self)->u8{{self.n}}}}fn bad()->u8{{let mut s=S{{n:1}};{tail}}}"
        );
        let error = check(&format!("borrow_bad_{i}"), &source)
            .err()
            .expect(&source);
        assert!(error.contains("L0286") || error.contains("E050"), "{error}");
    }
    run(
        "borrow_release",
        r#"
trait Read{fn read(&self)->u8;}struct S{n:u8}
impl Read for S{fn read(&self)->u8{self.n}}
pub fn answer()->u8{let mut s=S{n:1};let r:&dyn Read=&s;let n=r.read();s.n=2;n+s.n}
"#,
        "3",
    );
}

#[test]
#[doc = "spec: 1.31:56"]
fn exports_cannot_expose_internal_object_identity() {
    let checked = check("export_bad", "trait A{}pub fn f(r:&dyn A){}").unwrap();
    let error = project::rust(checked).expect_err("export must fail");
    assert!(error.to_string().contains("L0504"));
}

#[test]
fn shared_object_results_and_fields_retain_input_lifetimes() {
    run(
        "lifetimes",
        r#"
trait Read{fn read(&self)->u8;}
struct S{n:u8}impl Read for S{fn read(&self)->u8{self.n}}
struct Holder<'a>{object:&'a dyn Read}
fn erase<'a>(s:&'a S)->&'a dyn Read{&s}
fn pack<'a>(s:&'a S)->Holder<'a>{Holder{object:erase(&s)}}
pub fn answer()->u8{let s=S{n:71};let h=pack(&s);h.object.read()}
"#,
        "71",
    );
}

#[test]
#[doc = "spec: 1.31:57"]
fn object_escape_and_effect_promises_are_rejected() {
    for (i, body) in [
        "fn bad<'a>(s:&'a S)->&'a dyn Read{let local=S{n:1};&local}",
        "#[no_panic] fn bad(r:&dyn Read)->u8{r.read()}",
        "#[terminates] fn bad(r:&dyn Read)->u8{r.read()}",
        "logic fn bad(r:&dyn Read)->Int{0}",
    ]
    .iter()
    .enumerate()
    {
        let source = format!(
            "trait Read{{fn read(&self)->u8;}}struct S{{n:u8}}impl Read for S{{fn read(&self)->u8{{self.n}}}}{body}"
        );
        assert!(
            check(&format!("escape_effect_{i}"), &source).is_err(),
            "{source}"
        );
    }
}

#[test]
#[doc = "spec: 2.1:16"]
#[doc = "spec: 3.12:3"]
#[doc = "spec: 3.12:4"]
#[doc = "spec: 3.12:5"]
fn independent_checkers_reject_forged_tables_and_values() {
    use locus::{erased, exec::*, kernel::*};
    let mut definitions = Definitions::new();
    let concrete = definitions.declare_struct(&Type::Tuple(vec![])).unwrap();
    let mut program = Program::new(definitions);
    let interface = program
        .declare_dyn_interface("Empty".into(), vec![])
        .unwrap();
    let mut ctx = Context::with_definitions(std::rc::Rc::new(program.definitions().clone()));
    assert!(infer_term(&mut ctx, &Term::Struct(interface, vec![]), Mode::Logical).is_err());
    assert!(
        infer_term(
            &mut ctx,
            &Term::Instance(Box::new(Term::Struct(interface, vec![])), vec![]),
            Mode::Logical
        )
        .is_err()
    );
    assert!(
        program
            .definitions_mut()
            .mark_logical(&Type::Struct(interface))
            .is_err()
    );
    assert!(
        program
            .declare_dyn_table(DynTable {
                interface,
                concrete: Type::Struct(interface),
                methods: vec![]
            })
            .is_err()
    );
    let table = program
        .declare_dyn_table(DynTable {
            interface,
            concrete: Type::Struct(concrete),
            methods: vec![],
        })
        .unwrap();
    assert!(
        program
            .declare(ExecFn {
                promises: Promises::default(),
                signature: Type::Fn(vec![], Box::new(Type::Struct(interface))),
                params: vec![],
                body: Block {
                    stmts: vec![],
                    tail: Tail::DynPack {
                        table,
                        value: Term::U8(0)
                    }
                }
            })
            .is_err()
    );
    assert!(
        program
            .declare_dyn_interface(
                "Logical".into(),
                vec![DynMethod {
                    name: "bad".into(),
                    params: vec![],
                    result: Type::Prop
                }]
            )
            .is_err()
    );
    let opaque = ctx.declare(Type::Struct(interface)).unwrap();
    assert!(
        infer_term(
            &mut ctx,
            &Term::Proj(Box::new(Term::Free(opaque)), 0),
            Mode::Logical
        )
        .is_err()
    );
    let receiver = VarId::fresh();
    let one = program
        .declare(ExecFn {
            promises: Promises::default(),
            signature: Type::Fn(vec![Type::Struct(concrete)], Box::new(Type::U8)),
            params: vec![receiver],
            body: Block {
                stmts: vec![],
                tail: Tail::Value(Term::U8(1)),
            },
        })
        .unwrap();
    let reader = program
        .declare_dyn_interface(
            "Reader".into(),
            vec![DynMethod {
                name: "read".into(),
                params: vec![],
                result: Type::U8,
            }],
        )
        .unwrap();
    program
        .declare_dyn_table(DynTable {
            interface: reader,
            concrete: Type::Struct(concrete),
            methods: vec![one],
        })
        .unwrap();
    assert!(
        program
            .declare_dyn_table(DynTable {
                interface: reader,
                concrete: Type::Struct(concrete),
                methods: vec![one]
            })
            .is_err()
    );
    let wrong = program
        .declare_dyn_interface(
            "Wrong".into(),
            vec![DynMethod {
                name: "read".into(),
                params: vec![Type::U8],
                result: Type::U8,
            }],
        )
        .unwrap();
    assert!(
        program
            .declare_dyn_table(DynTable {
                interface: wrong,
                concrete: Type::Struct(concrete),
                methods: vec![one]
            })
            .is_err()
    );
    for (slot, arguments, result, promises) in [
        (1, vec![], Type::U8, Promises::default()),
        (0, vec![Term::U8(2)], Type::U8, Promises::default()),
        (0, vec![], Type::Bool, Promises::default()),
        (
            0,
            vec![],
            Type::U8,
            Promises {
                no_panic: true,
                ..Default::default()
            },
        ),
    ] {
        assert!(
            program
                .declare(ExecFn {
                    promises,
                    signature: Type::Fn(vec![Type::Struct(reader)], Box::new(result)),
                    params: vec![receiver],
                    body: Block {
                        stmts: vec![],
                        tail: Tail::DynCall {
                            interface: reader,
                            slot,
                            receiver: Term::Free(receiver),
                            arguments
                        }
                    },
                })
                .is_err()
        );
    }
    let c = check(
        "forged",
        &format!("{TWO}pub fn answer()->u8{{choose(true)}}"),
    )
    .unwrap();
    let module = c.checked.session.erased();
    erased::check_module(module).unwrap();
    let mut bad = module.clone();
    let opaque = bad.dynamics[0].id;
    bad.fns[0].result = erased::EType::Struct(opaque);
    assert!(erased::check_module(&bad).is_err());
    let mut bad = module.clone();
    bad.dynamics[0].methods[0].result = erased::EType::Ghost;
    assert!(erased::check_module(&bad).is_err());
    let mut bad = module.clone();
    bad.dyn_tables[0].methods.pop();
    assert!(erased::check_module(&bad).is_err());
    let mut bad = module.clone();
    bad.dyn_tables[0].concrete = erased::EType::Ghost;
    assert!(erased::check_module(&bad).is_err());
    let mut bad = module.clone();
    let table = bad.dyn_tables[0].clone();
    bad.dyn_tables.push(table);
    assert!(erased::check_module(&bad).is_err());
    let mut bad = module.clone();
    assert!(bad.dyn_tables.len() >= 2);
    bad.dyn_tables[1].concrete = bad.dyn_tables[0].concrete.clone();
    bad.dyn_tables[1].methods = bad.dyn_tables[0].methods.clone();
    assert!(erased::check_module(&bad).is_err());
    let mut bad = module.clone();
    let f = bad.dyn_tables[0].methods[0];
    bad.fns
        .iter_mut()
        .find(|x| x.reference == f)
        .unwrap()
        .passing[0] = locus::typed::Passing::Value;
    assert!(erased::check_module(&bad).is_err());
}

#[test]
fn argument_effects_are_evaluated_once() {
    run(
        "effects",
        r#"
trait Apply{fn apply(&self,n:u8)->u8;}
struct S{}impl Apply for S{fn apply(&self,n:u8)->u8{n}}
fn bump(n:&mut u8)->u8{n=n+1;n}
fn use_it(r:&dyn Apply)->(u8,u8){let mut n:u8=0;let result=r.apply(bump(&mut n));(result,n)}
pub fn answer()->(u8,u8){let s=S{};use_it(&s)}
"#,
        "(1, 1)",
    );
}

#[test]
#[doc = "spec: 1.31:57"]
fn unit_dispatch_preserves_panics() {
    let checked = check(
        "unit_panic",
        r#"
trait Action{fn act(&self);}
struct Stop{}impl Action for Stop{fn act(&self){panic!("stopped")}}
fn use_it(r:&dyn Action)->u8{r.act();9}
pub fn answer()->u8{let s=Stop{};use_it(&s)}
"#,
    )
    .unwrap();
    let session = &checked.checked.session;
    let f = checked
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("_answer"))
        .unwrap()
        .1;
    let expected = locus::erased::Outcome::Panic("stopped".into());
    assert_eq!(
        locus::exec::CheckInterpreter::new(session.program(), 10000)
            .call(f, vec![])
            .unwrap(),
        expected
    );
    assert_eq!(
        locus::erased::Interpreter::new(session.erased(), 10000)
            .call(f, vec![])
            .unwrap(),
        expected
    );
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("dyn_unit_panic");
    std::fs::write(dir.join("main.rs"),format!("{rust}\nfn main(){{std::panic::set_hook(Box::new(|_|{{}}));assert!(std::panic::catch_unwind(answer).is_err());}}")).unwrap();
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
fn generated_dispatch_cases_agree() {
    let mut seed = 0x716d_u64;
    for i in 0..8 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let (a, b, k) = ((seed >> 32) as u8, (seed >> 40) as u8, (seed >> 48) as u8);
        let text = format!(
            r#"
trait Apply{{fn apply(&self,n:u8)->u8;}}
struct A(u8);struct B(u8);
impl Apply for A{{fn apply(&self,n:u8)->u8{{self.0.wrapping_add(n)}}}}
impl Apply for B{{fn apply(&self,n:u8)->u8{{self.0.wrapping_sub(n)}}}}
enum Pick{{Left,Right}}
fn call(flag:Pick)->u8{{let a=A({a});let b=B({b});let r:&dyn Apply=match flag{{Pick::Left=>&a,Pick::Right=>&b}};r.apply({k})}}
pub fn answer()->(u8,u8){{(call(Pick::Left),call(Pick::Right))}}
"#
        );
        run(
            &format!("generated{i}"),
            &text,
            &format!("({}, {})", a.wrapping_add(k), b.wrapping_sub(k)),
        );
    }
}

#[test]
fn unsized_values_and_generic_dyn_implementations_fail_cleanly() {
    for (i,source) in [
        "trait A{}fn bad(r:&dyn A){let value=*r;}",
        "trait A{}struct S<T>{value:T}impl<T> A for S<T>{}fn f(r:&dyn A){}fn bad(){let s=S{value:1u8};f(&s)}",
        "trait A{}#[derive(Logical)]struct S{}impl A for S{}fn f(r:&dyn A){}fn bad(){let s=S{};f(&s)}",
    ].iter().enumerate() {
        assert!(check(&format!("unsized{i}"), source).is_err(),"{source}");
    }
}

#[test]
#[doc = "spec: 1.31:52"]
fn multifile_interfaces_aliases_and_enum_implementations() {
    let tag = "multifile";
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dyn_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, text) in [
        (
            "api.lc",
            "pub trait Read { fn read(&self)->u8; } pub enum Either { First(u8), Last(u8) } impl Read for Either { fn read(&self)->u8 { match self { Either::First(n)=>n, Either::Last(n)=>n } } }",
        ),
        (
            "consumer.lc",
            "use crate::api::Read as Reader; pub fn read(r:&(dyn Reader))->u8 {r.read()}",
        ),
        (
            "export.lc",
            "mod api;mod consumer; pub fn answer()->(u8,u8){let a=api::Either::First(3);let b=api::Either::Last(7);(consumer::read(&a),consumer::read(&b))}",
        ),
    ] {
        std::fs::write(dir.join(name), text).unwrap();
    }
    let checked = project::check(&dir.join("export.lc"), &Default::default()).unwrap();
    run_checked(tag, checked, "(3, 7)");
}

const READER: &str = r#"
trait Read { fn read(&self) -> u8; }
struct S { n: u8 }
impl Read for S { fn read(&self) -> u8 { self.n } }
"#;

#[test]
#[doc = "spec: 1.31:51"]
fn multiple_associated_bindings_are_order_independent() {
    run(
        "associated_order",
        r#"
trait Pair { type A; type B; fn pair(&self)->(Self::A,Self::B); }
struct S{} impl Pair for S { type A=u8;type B=bool;fn pair(&self)->(u8,bool){(4,true)} }
fn forward(r:&dyn Pair<A=u8,B=bool>)->(u8,bool){r.pair()}
pub fn answer()->(u8,bool){let s=S{};let r:&dyn Pair<B=bool,A=u8>=&s;forward(r)}
"#,
        "(4, true)",
    );
}

#[test]
#[doc = "spec: 1.31:53"]
fn same_method_name_on_distinct_traits_keeps_dispatch_identity() {
    run(
        "trait_identity",
        r#"
trait First {fn value(&self)->u8;} trait Second {fn value(&self)->u8;}
struct S{} impl First for S{fn value(&self)->u8{3}} impl Second for S{fn value(&self)->u8{8}}
pub fn answer()->(u8,u8){let s=S{};let a:&dyn First=&s;let b:&dyn Second=&s;(a.value(),b.value())}
"#,
        "(3, 8)",
    );
    let error = check(
        "trait_identity_reject",
        r#"
trait First{}trait Second{}fn consume(r:&dyn First){}fn wrong(r:&dyn Second){consume(r)}
"#,
    )
    .err()
    .expect("an unrelated object interface must not coerce");
    assert!(error.contains("L0518"), "{error}");
}

#[test]
#[doc = "spec: 1.31:53"]
fn shared_dispatch_survives_loop_state_and_tuple_destructuring() {
    run(
        "loop_tuple",
        &format!(
            r#"{READER}
fn sum(r:&dyn Read)->u8{{let pair=(r,r);let (left,right)=pair;let mut i:u8=0;let mut total:u8=0;while i<3{{total=total+left.read();i=i+1;}}total+right.read()}}
pub fn answer()->u8{{let s=S{{n:6}};sum(&s)}}
"#
        ),
        "24",
    );
}

#[test]
#[doc = "spec: 1.31:54"]
fn object_identity_and_branching_results_keep_input_lifetime() {
    run(
        "object_return",
        &format!(
            r#"{READER}
fn identity<'a>(r:&'a dyn Read)->&'a dyn Read{{r}}
fn choose<'a>(flag:bool,a:&'a S,b:&'a S)->&'a dyn Read{{if flag{{&a}}else{{&b}}}}
pub fn answer()->(u8,u8){{let a=S{{n:3}};let b=S{{n:9}};(identity(choose(true,&a,&b)).read(),identity(choose(false,&a,&b)).read())}}
"#
        ),
        "(3, 9)",
    );
}

#[test]
#[doc = "spec: 1.31:54"]
fn object_fields_and_nested_results_do_not_hide_invalid_borrows() {
    for (i, body) in [
        "fn bad<'a,'b>(a:&'a S,b:&'b S)->&'a dyn Read{&b}",
        "struct Holder<'a>{r:&'a dyn Read}fn bad()->u8{let mut s=S{n:1};let h=Holder{r:&s};s.n=2;h.r.read()}",
        "struct Holder<'a>{r:&'a dyn Read}fn bad<'a>(s:&'a S)->Holder<'a>{let t=S{n:1};Holder{r:&t}}",
        "fn bad()->u8{let mut a=S{n:1};let b=S{n:2};let r:&dyn Read=if true{&a}else{&b};a.n=3;r.read()}",
    ].iter().enumerate() {
        let error=check(&format!("nested_borrow_bad{i}"),&format!("{READER}{body}")).err().expect("borrow must fail");
        assert!(error.contains("L0286") || error.contains("E050"),"{body}\n{error}");
    }
}

#[test]
#[doc = "spec: 1.31:54"]
fn disjoint_field_mutation_preserves_a_live_object_borrow() {
    run(
        "disjoint",
        &format!(
            r#"{READER}
struct Pair{{left:S,right:S}}
pub fn answer()->u8{{let mut pair=Pair{{left:S{{n:4}},right:S{{n:8}}}};let r:&dyn Read=&pair.left;pair.right.n=11;r.read()+pair.right.n}}
"#
        ),
        "15",
    );
}

#[test]
#[doc = "spec: 1.31:56"]
fn exports_reject_objects_nested_in_all_public_interface_shapes() {
    for (i, item) in [
        "pub struct Holder<'a>{pub r:&'a dyn Read}",
        "pub enum Holder<'a>{Some(&'a dyn Read)}",
        "pub fn get<'a>(s:&'a S)->(u8,&'a dyn Read){(1,&s)}",
        "pub trait API{fn use_it(&self,r:&dyn Read);}",
        "pub struct Holder{}impl Holder{pub fn get(&self,r:&dyn Read)->u8{r.read()}}",
    ]
    .iter()
    .enumerate()
    {
        let checked = check(&format!("nested_export{i}"), &format!("{READER}{item}")).unwrap();
        let error = project::rust(checked)
            .expect_err("export must reject object leakage")
            .to_string();
        assert!(error.contains("L0504"), "{item}\n{error}");
    }
}

#[test]
#[doc = "spec: 1.31:57"]
fn dyn_callers_can_prove_and_replay_properties_of_returned_data() {
    let tag = "caller_proofs";
    let text = format!(
        r#"{READER}
fn measured(r:&dyn Read)->(out:u8,@(out<=255)){{let out=r.read();(out,prove!(out<=255))}}
pub fn answer()->u8{{let s=S{{n:17}};let (value,evidence)=measured(&s);value}}
"#
    );
    run(tag, &text, "17");
    // Outside the compiler's Cargo package: a package owns its proof lockfile.
    let dir = std::env::temp_dir().join(format!("locus-dyn-proof-replay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("export.lc"), text).unwrap();
    let lock = dir.join("Locus.lock");
    if lock.exists() {
        std::fs::remove_file(&lock).unwrap();
    }
    let first = std::process::Command::new(env!("CARGO_BIN_EXE_locus"))
        .current_dir(&dir)
        .env_remove("LOCUS_PROOFS")
        .args(["check", "export.lc"])
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let before = std::fs::read(&lock).expect("recorded proof lockfile");
    let locked = std::process::Command::new(env!("CARGO_BIN_EXE_locus"))
        .current_dir(&dir)
        .env_remove("LOCUS_PROOFS")
        .env("LOCUS_SEARCH", "none")
        .args(["check", "export.lc", "--locked"])
        .output()
        .unwrap();
    assert!(
        locked.status.success(),
        "{}",
        String::from_utf8_lossy(&locked.stderr)
    );
    assert_eq!(before, std::fs::read(&lock).unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[doc = "spec: 1.31:57"]
fn dynamic_argument_order_and_short_circuiting_preserve_effects() {
    run(
        "ordered_effects",
        r#"
trait Apply{fn pair(&self,a:u8,b:u8)->u8;fn fail(&self)->bool;}
struct S{}impl Apply for S{fn pair(&self,a:u8,b:u8)->u8{a*10+b}fn fail(&self)->bool{panic!("unexpected call")}}
fn bump(n:&mut u8)->u8{n=n+1;n}
fn call(r:&dyn Apply,flag:bool)->(u8,u8,bool,bool){let mut n:u8=0;let pair=r.pair(bump(&mut n),bump(&mut n));(pair,n,flag&&r.fail(),!flag||r.fail())}
pub fn answer()->(u8,u8,bool,bool){let s=S{};call(&s,false)}
"#,
        "(12, 2, false, true)",
    );
}

#[test]
#[doc = "spec: 1.31:50"]
fn concrete_methods_are_not_available_through_the_object_interface() {
    let error = check(
        "hidden_method",
        &format!(
            r#"{READER}
impl S{{fn secret(&self)->u8{{99}}}}fn bad(r:&dyn Read)->u8{{r.secret()}}
"#
        ),
    )
    .err()
    .expect("concrete-only method must be hidden");
    assert!(error.contains("secret"), "{error}");
}

#[test]
#[doc = "spec: 3.12:5"]
fn erased_dispatch_expressions_reject_corrupt_operations_and_operands() {
    use locus::erased::{self, DynOperation, EExpr};
    let checked = check(
        "erased_corruption",
        &format!("{TWO}pub fn answer()->u8{{choose(true)}}"),
    )
    .unwrap();
    let module = checked.checked.session.erased();
    for mode in 0..6 {
        let mut bad = module.clone();
        let pack = mode < 2;
        let expression=bad.fns.iter_mut().filter_map(|f|f.body.tail.as_deref_mut()).find(|e| matches!(e,EExpr::Dynamic{operation,..} if matches!(operation,DynOperation::Pack(_))==pack)).unwrap();
        let EExpr::Dynamic {
            operation,
            arguments,
        } = expression
        else {
            unreachable!()
        };
        match mode {
            0 => arguments.clear(),
            1 => arguments[0] = EExpr::Bool(true),
            2 => {
                if let DynOperation::Call { slot, .. } = operation {
                    *slot = usize::MAX;
                }
            }
            3 => arguments.clear(),
            4 => arguments[0] = EExpr::Bool(false),
            5 => arguments.push(EExpr::Bool(true)),
            _ => unreachable!(),
        }
        assert!(
            erased::check_module(&bad).is_err(),
            "accepted corruption {mode}"
        );
    }
    let mut bad = module.clone();
    let missing = bad.dyn_tables[0].methods[0];
    bad.fns.retain(|f| f.reference != missing);
    assert!(erased::check_module(&bad).is_err());
}

#[test]
#[doc = "spec: 1.31:57"]
fn dynamic_calls_do_not_invent_observer_equations_or_reuse_result_evidence() {
    for (i, body) in [
        "fn bad(r:&dyn Read)->(out:u8,@(out==17)){let out=r.read();(out,prove!(out==17))}",
        "fn bad(r:&dyn Read)->@(true){let a=r.read();let p:@(a==a)=prove!(a==a);let b=r.read();let q:@(b==a)=p;prove!(true)}",
    ].iter().enumerate() {
        let error=check(&format!("no_observer_{i}"),&format!("{READER}{body}")).err().expect("unestablished claim must fail");
        assert!(error.contains("L0230") || error.contains("L0201") || error.contains("L0220"),"{body}\n{error}");
    }
}

#[test]
#[doc = "spec: 1.31:51"]
fn pointer_sized_slots_are_checked_for_the_selected_target() {
    use locus::{
        elab,
        erased::{self, Outcome, Value},
        kernel::PointerWidth,
    };
    let code = r#"
trait Platform{fn adjust(&self,n:usize,s:isize)->(usize,isize);}
struct S{}impl Platform for S{fn adjust(&self,n:usize,s:isize)->(usize,isize){(n+1,s-1)}}
fn use_it(r:&dyn Platform)->(usize,isize){r.adjust(41usize,-3isize)}
pub fn answer()->(usize,isize){let s=S{};use_it(&s)}
"#;
    for width in [PointerWidth::W32, PointerWidth::W64] {
        let dir =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dyn_target_{}", width.bits()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("export.lc"), code).unwrap();
        let checked = project::check(
            &dir.join("export.lc"),
            &elab::Options {
                pointer_width: width,
                ..Default::default()
            },
        )
        .unwrap();
        let session = &checked.checked.session;
        let function = checked
            .checked
            .functions
            .iter()
            .find(|(n, _)| n.ends_with("_answer"))
            .unwrap()
            .1;
        let a = locus::exec::CheckInterpreter::new(session.program(), 10000)
            .with_lending(session.lending())
            .call(function, vec![])
            .unwrap();
        let b = erased::Interpreter::new(session.erased(), 10000)
            .call(function, vec![])
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a,
            Outcome::Value(Value::Tuple(vec![
                Value::Int(width.usize(), 42),
                Value::Int(width.isize(), -4)
            ]))
        );
        let mut bad = session.erased().clone();
        bad.pointer_width = if width == PointerWidth::W32 {
            PointerWidth::W64
        } else {
            PointerWidth::W32
        };
        assert!(erased::check_module(&bad).is_err());
        let rust = project::rust(checked).unwrap();
        assert!(rust.contains(&format!("target_pointer_width = \"{}\"", width.bits())));
    }
    run("target_host", code, "(42, -4)");
}

#[test]
#[doc = "spec: 1.31:52"]
fn repeated_coercions_reuse_the_table_but_not_the_concrete_value() {
    let source = format!(
        r#"{READER}
fn first(s:&S)->u8{{let r:&dyn Read=&s;r.read()}}
fn second(s:&S)->u8{{let r=&s as &dyn Read;r.read()}}
pub fn answer()->(u8,u8){{let a=S{{n:2}};let b=S{{n:7}};(first(&a),second(&b))}}
"#
    );
    let checked = check("repeated_coercions", &source).unwrap();
    assert_eq!(checked.checked.session.erased().dyn_tables.len(), 1);
    run_checked("repeated_coercions", checked, "(2, 7)");
}

#[test]
#[doc = "spec: 1.31:53"]
fn object_reference_reassignment_and_loop_joins_keep_the_selected_receiver() {
    run(
        "reassign_loop",
        &format!(
            r#"{READER}
pub fn answer()->(u8,u8){{let a=S{{n:2}};let b=S{{n:8}};let mut r:&dyn Read=&a;let first=r.read();let mut i:u8=0;while i<2{{r=&b;i=i+1;}}(first,r.read())}}
"#
        ),
        "(2, 8)",
    );
}

#[test]
#[doc = "spec: 1.31:54"]
fn enum_payloads_preserve_object_references_and_reject_local_escape() {
    let prefix = format!(
        r#"{READER}
enum Maybe<'a>{{None,Some(&'a dyn Read)}}
fn make<'a>(s:&'a S)->Maybe<'a>{{Maybe::Some(&s)}}
fn get(m:Maybe)->u8{{match m{{Maybe::None=>0,Maybe::Some(r)=>r.read()}}}}
"#
    );
    run(
        "enum_storage",
        &format!("{prefix}pub fn answer()->u8{{let s=S{{n:32}};get(make(&s))}}"),
        "32",
    );
    let error=check("enum_escape",&format!("{READER}enum Maybe<'a>{{Some(&'a dyn Read)}}fn bad<'a>(s:&'a S)->Maybe<'a>{{let t=S{{n:3}};Maybe::Some(&t)}}")).err().expect("enum must not hide local escape");
    assert!(error.contains("L0286"), "{error}");
}

#[test]
#[doc = "spec: 1.31:53"]
fn dynamic_selection_agrees_for_all_byte_receivers_and_boundary_arguments() {
    use locus::{
        erased::{Outcome, Value},
        kernel::MachineInt,
    };
    let tag = "all_bytes";
    let checked=check(tag,r#"
trait Apply{fn apply(&self,n:u8)->u8;}
struct A(u8);struct B(u8);
impl Apply for A{fn apply(&self,n:u8)->u8{self.0.wrapping_add(n)}}
impl Apply for B{fn apply(&self,n:u8)->u8{self.0.wrapping_sub(n)}}
pub fn answer(flag:bool,a:u8,b:u8,k:u8)->u8{let a=A(a);let b=B(b);let r:&dyn Apply=if flag{&a}else{&b};r.apply(k)}
"#).unwrap();
    let session = &checked.checked.session;
    let f = checked
        .checked
        .functions
        .iter()
        .find(|(n, _)| n.ends_with("_answer"))
        .unwrap()
        .1;
    for a in 0u8..=255 {
        let b = 255 - a;
        for k in [0u8, 1, 127, 255] {
            for flag in [false, true] {
                let args = vec![
                    Value::Bool(flag),
                    Value::Int(MachineInt::U8, a.into()),
                    Value::Int(MachineInt::U8, b.into()),
                    Value::Int(MachineInt::U8, k.into()),
                ];
                let expected = Outcome::Value(Value::Int(
                    MachineInt::U8,
                    if flag {
                        a.wrapping_add(k)
                    } else {
                        b.wrapping_sub(k)
                    }
                    .into(),
                ));
                let checked_result = locus::exec::CheckInterpreter::new(session.program(), 10000)
                    .with_lending(session.lending())
                    .call(f, args.clone())
                    .unwrap();
                let erased_result = locus::erased::Interpreter::new(session.erased(), 10000)
                    .call(f, args)
                    .unwrap();
                assert_eq!(checked_result, expected, "check: {flag}, {a}, {b}, {k}");
                assert_eq!(erased_result, expected, "erased: {flag}, {a}, {b}, {k}");
            }
        }
    }
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dyn_{tag}"));
    std::fs::write(dir.join("main.rs"),format!("{rust}\nfn main(){{for a in 0u8..=255{{let b=255-a;for k in [0u8,1,127,255]{{for flag in [false,true]{{assert_eq!(answer(flag,a,b,k),if flag{{a.wrapping_add(k)}}else{{b.wrapping_sub(k)}});}}}}}}}}")).unwrap();
    for checks in ["yes", "no"] {
        let out = std::process::Command::new("rustc")
            .args(["--edition=2024", "-Dwarnings", "-C"])
            .arg(format!("overflow-checks={checks}"))
            .arg(dir.join("main.rs"))
            .arg("-o")
            .arg(dir.join("app"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            std::process::Command::new(dir.join("app"))
                .status()
                .unwrap()
                .success()
        );
    }
}
