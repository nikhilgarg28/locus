//! The independent erased checker sees logical Bool fields even though the
//! proof kernel represents Bool and bool using the same mathematical type.
use locus::{
    erased::{self, EType, Interpreter, Outcome, Value},
    exec::Promises,
    kernel::{Definitions, Type},
    typed::{Binder, Block, ErasureLayout as Layout, Expr, FnItem, Session, StructItem},
};

fn tuple(types: Vec<Type>) -> Type {
    Type::Tuple(types)
}

fn function(name: &str, param: Binder, result: Type, tail: Expr) -> FnItem {
    FnItem {
        name: name.into(),
        math: false,
        params: vec![param],
        result,
        body: Block {
            stmts: vec![],
            tail: Some(Box::new(tail)),
        },
        passing: vec![],
        exits: vec![],
    }
}

#[test]
#[doc = "spec: 3.5:1"]
fn mixed_tuple_layout_survives_parameter_and_return() {
    let mut session = Session::new(Definitions::default());
    let ty = tuple(vec![Type::Bool, tuple(vec![Type::Bool, Type::U8])]);
    let param = Binder::new("packet", ty.clone());
    let layout = Layout::Tuple(vec![
        Layout::Logical,
        Layout::Tuple(vec![Layout::Default, Layout::Default]),
    ]);
    session.register_binding_layout(param.id, layout.clone());
    let item = function("identity", param.clone(), ty, Expr::var(&param));
    let id = session
        .declare_fn_with_layout(&item, Promises::default(), layout.clone())
        .unwrap();
    assert_eq!(session.function_result_layout(id), layout);
    let function = &session.erased().fns[0];
    let expected = EType::Tuple(vec![
        EType::Ghost,
        EType::Tuple(vec![EType::Bool, EType::Int(locus::kernel::MachineInt::U8)]),
    ]);
    assert_eq!(function.params[0].2, expected);
    assert_eq!(function.result, expected);
    assert_eq!(erased::check_module(session.erased()), Ok(()));
    let input = Value::Tuple(vec![
        Value::Ghost,
        Value::Tuple(vec![Value::Bool(true), Value::u8(7)]),
    ]);
    assert_eq!(
        Interpreter::new(session.erased(), 10_000)
            .call(id, vec![input.clone()])
            .unwrap(),
        Outcome::Value(input)
    );
}

#[test]
fn the_erased_checker_rejects_a_logical_tuple_projection_as_runtime_bool() {
    let mut session = Session::new(Definitions::default());
    let ty = tuple(vec![Type::Bool]);
    let param = Binder::new("packet", ty);
    session.register_binding_layout(param.id, Layout::Tuple(vec![Layout::Logical]));
    let field = Expr::Field {
        target: Box::new(Expr::var(&param)),
        index: 0,
        name: None,
        ty: Type::Bool,
    };
    assert_eq!(session.expression_layout(&field), Layout::Logical);
    let item = function("leak", param, Type::Bool, field);
    session.declare_fn(&item).unwrap();
    assert!(erased::check_module(session.erased()).is_err());
}

#[test]
fn struct_field_layout_is_available_through_a_projection() {
    let mut session = Session::new(Definitions::default());
    let field = Binder::new("pair", tuple(vec![Type::Bool, Type::U8]));
    let layout = Layout::Tuple(vec![Layout::Logical, Layout::Default]);
    session.register_binding_layout(field.id, layout.clone());
    let id = session
        .declare_struct(&StructItem {
            shape: locus::ast::VariantShape::Struct,
            name: "Packet".into(),
            fields: vec![field.clone()],
            derives: vec![],
        })
        .unwrap();
    let parameter = Binder::new("packet", Type::Struct(id));
    let pair = Expr::Field {
        target: Box::new(Expr::var(&parameter)),
        index: 0,
        name: Some("pair".into()),
        ty: field.ty.clone(),
    };
    assert_eq!(session.expression_layout(&pair), layout);
    let logical = Expr::Field {
        target: Box::new(pair),
        index: 0,
        name: None,
        ty: Type::Bool,
    };
    assert_eq!(session.expression_layout(&logical), Layout::Logical);
    assert_eq!(
        session.erased().structs[0].fields[0].1,
        EType::Tuple(vec![
            EType::Ghost,
            EType::Int(locus::kernel::MachineInt::U8)
        ])
    );
}

#[test]
#[doc = "spec: 3.5:2"]
fn erasing_a_logical_result_preserves_which_runtime_branch_mutates() {
    use locus::{
        elab::{Options, elaborate_with_options},
        preview::{Feature, Status},
        source::SourceMap,
    };
    let mut sources = SourceMap::default();
    let file = sources.add(
        "branch.lc",
        "
fn choose(flag: bool, value: &mut u8) -> Bool {
    if flag { value = 1; true } else { value = 2; false }
}
fn run(flag: bool) -> u8 {
    let mut value = 0u8;
    let evidence = choose(flag, &mut value);
    value
}",
    );
    let source = sources.get(file);
    let parsed = locus::parser::parse(source);
    assert!(parsed.is_success(), "{:?}", parsed.diagnostics);
    let mut options = Options::default();
    if Feature::LogicalSplit.status() == Status::Preview {
        options.previews.enable("logical-split").unwrap();
    }
    let checked = elaborate_with_options(source, &parsed.program, &options);
    assert!(checked.is_success(), "{:?}", checked.diagnostics);
    assert_eq!(erased::check_module(checked.session.erased()), Ok(()));
    let run = checked.function("run").unwrap();
    for (flag, expected) in [(true, 1), (false, 2)] {
        assert_eq!(
            Interpreter::new(checked.session.erased(), 10_000)
                .call(run, vec![Value::Bool(flag)])
                .unwrap(),
            Outcome::Value(Value::u8(expected))
        );
    }
}
