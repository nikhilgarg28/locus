//! Independently checked native collection contracts and runtime agreement.
use locus::{
    erased::{self, Interpreter, Outcome, Value},
    exec::{BufferStorage, CheckInterpreter},
    kernel::{BufferOp, Definitions, MachineInt, Type},
    typed::{BufferFunction, ErasureLayout, Session},
};
use std::process::Command;

fn helper(
    session: &mut Session,
    name: &str,
    op: BufferOp,
    storage: BufferStorage,
    count: usize,
) -> BufferFunction {
    session.declare_buffer_function(name.into(), op, storage, Type::U8, ErasureLayout::Default, count,
        "Rust standard-library collection operation; checked contents equation on normal return".into()).unwrap()
}
fn run(session: &Session, function: &BufferFunction, args: Vec<Value>) -> (Outcome, Vec<Value>) {
    erased::check_module(session.erased()).unwrap();
    let checked = CheckInterpreter::new(session.program(), 10000)
        .with_lending(session.lending())
        .call_lending(function.reference, args.clone())
        .unwrap();
    let erased = Interpreter::new(session.erased(), 10000)
        .call_lending(function.reference, args)
        .unwrap();
    assert_eq!(checked, erased);
    checked
}
fn bytes(values: &[u8]) -> Value {
    Value::Buffer(values.iter().copied().map(Value::u8).collect())
}
fn index(value: u64) -> Value {
    Value::Int(MachineInt::U64, i128::from(value))
}

#[test]
#[doc = "spec: 1.26:1, 2.35:5"]
#[doc = "spec: 3.2:8, 3.2:9"]
fn native_array_slice_and_vector_helpers_check_and_execute() {
    let mut session = Session::new(Definitions::default());
    let array = helper(
        &mut session,
        "make_array",
        BufferOp::Literal,
        BufferStorage::Array(3),
        3,
    );
    assert_eq!(
        run(
            &session,
            &array,
            vec![Value::u8(2), Value::u8(4), Value::u8(6)]
        )
        .0,
        Outcome::Value(Value::Tuple(vec![bytes(&[2, 4, 6]), Value::Proved]))
    );
    let len = helper(
        &mut session,
        "slice_len",
        BufferOp::Length,
        BufferStorage::Slice,
        0,
    );
    assert_eq!(
        run(&session, &len, vec![bytes(&[2, 4, 6])]).0,
        Outcome::Value(Value::Tuple(vec![index(3), Value::Proved]))
    );
    let get = helper(
        &mut session,
        "slice_get",
        BufferOp::Get,
        BufferStorage::Slice,
        0,
    );
    assert_eq!(
        run(
            &session,
            &get,
            vec![bytes(&[2, 4, 6]), index(1), Value::Proved, Value::Proved]
        )
        .0,
        Outcome::Value(Value::Tuple(vec![Value::u8(4), Value::Proved]))
    );
    let set = helper(
        &mut session,
        "slice_set",
        BufferOp::Set,
        BufferStorage::Slice,
        0,
    );
    assert_eq!(
        run(
            &session,
            &set,
            vec![
                bytes(&[2, 4, 6]),
                index(1),
                Value::u8(9),
                Value::Proved,
                Value::Proved
            ]
        ),
        (Outcome::Value(Value::Proved), vec![bytes(&[2, 9, 6])])
    );
    let push = helper(
        &mut session,
        "vector_push",
        BufferOp::Push,
        BufferStorage::Vector,
        0,
    );
    assert_eq!(
        run(&session, &push, vec![bytes(&[1, 2]), Value::u8(3)]),
        (
            Outcome::Value(Value::Tuple(vec![Value::Proved, Value::Proved])),
            vec![bytes(&[1, 2, 3])]
        )
    );
    assert!(!push.promises.no_alloc && !push.promises.no_panic);
    assert!(get.promises.no_panic && get.promises.no_alloc);
    assert_eq!(session.buffer_functions().len(), 5);
}

#[test]
fn native_helpers_emit_real_rust_and_are_private() {
    let mut session = Session::new(Definitions::default());
    helper(
        &mut session,
        "make_array",
        BufferOp::Literal,
        BufferStorage::Array(3),
        3,
    );
    helper(
        &mut session,
        "slice_len",
        BufferOp::Length,
        BufferStorage::Slice,
        0,
    );
    helper(
        &mut session,
        "slice_get",
        BufferOp::Get,
        BufferStorage::Slice,
        0,
    );
    helper(
        &mut session,
        "slice_set",
        BufferOp::Set,
        BufferStorage::Slice,
        0,
    );
    helper(
        &mut session,
        "vector_push",
        BufferOp::Push,
        BufferStorage::Vector,
        0,
    );
    let rust = erased::print_module(session.erased());
    assert!(!rust.contains("pub fn slice_get"), "{rust}");
    assert!(rust.contains("target.push(value)"), "{rust}");
    let path = std::env::temp_dir().join(format!("locus-collections-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let source = format!(
        "{rust}\nfn main() {{ let (mut a, _) = make_array(2,4,6); let (n, _) = slice_len(&a); let (v, _) = slice_get(&a,1,Erased,Erased); slice_set(&mut a,1,9,Erased,Erased); let mut b = vec![1,2]; vector_push(&mut b,3); println!(\"{{n}} {{v}} {{a:?}} {{b:?}}\"); }}"
    );
    std::fs::write(path.join("main.rs"), &source).unwrap();
    let output = Command::new("rustc")
        .args(["--edition=2024", "-Dwarnings", "main.rs", "-o", "run"])
        .current_dir(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = Command::new(path.join("run")).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        "3 4 [2, 9, 6] [1, 2, 3]"
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn native_registration_requires_a_reason_and_valid_storage_shape() {
    let mut session = Session::new(Definitions::default());
    for (op, storage, count, reason) in [
        (
            BufferOp::Literal,
            BufferStorage::Array(3),
            2,
            "literal length must match",
        ),
        (
            BufferOp::Literal,
            BufferStorage::Slice,
            0,
            "slices cannot allocate",
        ),
        (
            BufferOp::Push,
            BufferStorage::Slice,
            0,
            "slices cannot grow",
        ),
        (BufferOp::Length, BufferStorage::Slice, 0, ""),
    ] {
        assert!(
            session
                .declare_buffer_function(
                    "bad".into(),
                    op,
                    storage,
                    Type::U8,
                    ErasureLayout::Default,
                    count,
                    reason.into()
                )
                .is_err()
        );
    }
    assert!(session.erased().fns.is_empty());
}

#[test]
fn executable_checker_refuses_forged_bounds_and_stronger_effect_promises() {
    use locus::{
        exec::{Program, Stmt},
        kernel::{Proof, Term},
        typed::FnRef,
    };
    let mut session = Session::new(Definitions::default());
    let get = helper(&mut session, "read", BufferOp::Get, BufferStorage::Slice, 0);
    let FnRef::Exec(id) = get.reference else {
        unreachable!()
    };
    let valid = session.program().function(id).unwrap().clone();
    let mut missing = valid.clone();
    let Stmt::Buffer(operation) = &mut missing.body.stmts[0] else {
        unreachable!()
    };
    operation.bounds.clear();
    assert!(
        Program::new(Definitions::default())
            .declare(missing)
            .is_err()
    );
    let mut erased_payload = valid.clone();
    let Stmt::Buffer(operation) = &mut erased_payload.body.stmts[0] else {
        unreachable!()
    };
    operation.logical_payload = true;
    assert!(
        Program::new(Definitions::default())
            .declare(erased_payload)
            .is_err()
    );
    let mut wrong = valid;
    let Stmt::Buffer(operation) = &mut wrong.body.stmts[0] else {
        unreachable!()
    };
    operation.bounds[1] = Proof::Refl(Term::U8(0));
    assert!(Program::new(Definitions::default()).declare(wrong).is_err());
    let push = helper(
        &mut session,
        "append",
        BufferOp::Push,
        BufferStorage::Vector,
        0,
    );
    let FnRef::Exec(id) = push.reference else {
        unreachable!()
    };
    for no_panic in [false, true] {
        let mut stronger = session.program().function(id).unwrap().clone();
        if no_panic {
            stronger.promises.no_panic = true;
        } else {
            stronger.promises.no_alloc = true;
        }
        assert!(
            Program::new(Definitions::default())
                .declare(stronger)
                .is_err()
        );
    }
}

#[test]
#[doc = "spec: 3.2:8"]
fn typed_box_payload_erasure_cannot_be_forged() {
    use locus::{
        kernel::{HypId, VarId},
        typed::{Block, Expr, FnItem, LowerError},
    };
    for (value, logical_payload) in [
        (Expr::Bool(true), true),
        (Expr::Ghost(Box::new(Expr::Bool(true))), false),
    ] {
        let mut session = Session::new(Definitions::default());
        let item = FnItem {
            name: "forged_payload".into(),
            math: false,
            params: vec![],
            result: Type::Boxed(Box::new(Type::Bool)),
            passing: vec![],
            exits: vec![],
            body: Block {
                stmts: vec![],
                tail: Some(Box::new(Expr::BoxNew {
                    value: Box::new(value),
                    result: VarId::fresh(),
                    equation: HypId::fresh(),
                    logical_payload,
                })),
            },
        };
        let error = session.declare_fn(&item).unwrap_err();
        assert!(
            matches!(error, LowerError::ReferencePermission(ref why) if why.contains("erasure flag")),
            "{error:?}"
        );
        assert!(session.erased().fns.is_empty());
    }
}
