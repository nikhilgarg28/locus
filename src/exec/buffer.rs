//! The trusted connection between physical operations and immutable snapshots.
//! Every executable argument is checked before any logical fact is introduced.
use super::{BufferStmt, BufferStorage, ExecError, Promises};
use crate::kernel::buffer::{bounds, length};
use crate::kernel::{
    BufferOp, Context, MachineInt, Mode, Proof, Term, Type, check_proof, infer_term, same_type,
};

pub(super) fn check(
    ctx: &mut Context,
    operation: &BufferStmt,
    promises: Promises,
    definitions: &crate::kernel::Definitions,
) -> Result<(), ExecError> {
    let BufferStmt {
        var,
        equation,
        op,
        storage,
        element,
        logical_payload,
        arguments,
        bounds: evidence,
        learned,
    } = operation;
    let bad = ExecError::InvalidBuffer;
    if *logical_payload && !definitions.is_erased_type(element) && *element != Type::Bool {
        return Err(bad("physical collection payload cannot be silently erased"));
    }
    let erased_payload = *logical_payload || definitions.is_erased_type(element);
    if matches!(op, BufferOp::Push) && *storage != BufferStorage::Vector {
        return Err(bad("only vectors can grow"));
    }
    if matches!(op, BufferOp::Literal) && *storage == BufferStorage::Slice {
        return Err(bad("a slice must borrow existing storage"));
    }
    if let (BufferOp::Literal, BufferStorage::Array(n)) = (op, storage)
        && arguments.len() != *n
    {
        return Err(bad("array literal length does not match physical storage"));
    }
    let allocating = *op == BufferOp::Push
        || (*op == BufferOp::Literal && *storage == BufferStorage::Vector && !arguments.is_empty());
    if allocating && promises.no_alloc {
        return Err(bad("allocation under no_alloc"));
    }
    if allocating && promises.no_panic {
        return Err(bad("allocation/capacity failure under no_panic"));
    }
    let buffer = Type::Buffer(Box::new(element.clone()));
    let expected = match op {
        BufferOp::Literal => vec![element.clone(); arguments.len()],
        BufferOp::Length => vec![buffer.clone()],
        BufferOp::Get => vec![buffer.clone(), Type::machine(MachineInt::U64)],
        BufferOp::Set => vec![
            buffer.clone(),
            Type::machine(MachineInt::U64),
            element.clone(),
        ],
        BufferOp::Push => vec![buffer.clone(), element.clone()],
    };
    if arguments.len() != expected.len() {
        return Err(bad("wrong argument count"));
    }
    for (index, (argument, expected)) in arguments.iter().zip(expected).enumerate() {
        let payload = match op {
            BufferOp::Literal => true,
            BufferOp::Push => index == 1,
            BufferOp::Set => index == 2,
            BufferOp::Length | BufferOp::Get => false,
        };
        let mode = if payload && erased_payload {
            Mode::Logical
        } else {
            Mode::Executable
        };
        let found = infer_term(ctx, argument, mode)?;
        if !same_type(&found, &expected) {
            return Err(bad("wrong runtime argument type"));
        }
    }
    let indexing = matches!(op, BufferOp::Get | BufferOp::Set);
    if evidence.len() != if indexing { 2 } else { 0 } {
        return Err(bad("indexing requires exactly two bounds proofs"));
    }
    if learned.len() != usize::from(*op == BufferOp::Push) {
        return Err(bad("wrong normal-return facts"));
    }
    let mut model_arguments = arguments.clone();
    if indexing {
        let index = Term::view(MachineInt::U64, arguments[1].clone());
        let claims = bounds(element, &arguments[0], &index);
        for (proof, claim) in evidence.iter().zip(claims) {
            check_proof(ctx, proof, &claim)?;
        }
        model_arguments = vec![arguments[0].clone(), index];
        model_arguments.extend(evidence.iter().cloned().map(Term::proof));
        if *op == BufferOp::Set {
            model_arguments.push(arguments[2].clone());
        }
    }
    if *op == BufferOp::Push {
        let room = Term::int_lt(
            length(element.clone(), arguments[0].clone()),
            Term::Int(MachineInt::U64.max()),
        );
        // Reaching the next statement means Rust's push returned. It could
        // not have produced a representable collection with an overflowing length.
        ctx.assume_with(learned[0], room)?;
        model_arguments.push(Term::proof(Proof::hyp(learned[0])));
    }
    let model = Term::Buffer {
        op: *op,
        element: element.clone(),
        arguments: model_arguments,
    };
    let model_ty = infer_term(ctx, &model, Mode::Logical)?;
    let result_ty = if *op == BufferOp::Length {
        Type::machine(MachineInt::U64)
    } else {
        model_ty.clone()
    };
    ctx.declare_with(*var, result_ty, false)?;
    // Evidence is already usable at its checked proposition; the kernel
    // deliberately has no equality between proof objects.
    if matches!(model_ty, Type::Proof(_)) {
        return Ok(());
    }
    let actual = if *op == BufferOp::Length {
        Term::view(MachineInt::U64, Term::Free(*var))
    } else {
        Term::Free(*var)
    };
    ctx.assume_with(*equation, Term::eq(model_ty, actual, model))?;
    Ok(())
}
