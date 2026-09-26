//! Contract for physical collection snapshots (Reconciliation B1/B2).
//!
//! A buffer value is an immutable finite sequence of element snapshots. It
//! does not contain addresses, allocation capacity, borrows, or a Rust
//! layout. Arrays, parameter slices, and Vec values have different physical
//! layouts but may use the same content snapshot. Buffer identity is never
//! sufficient to authorize a borrow: the source permission checker tracks
//! the storage root, alias path, lifetime, and its current SSA version.
//!
//! `Buffer<T>` contains at most the selected `usize::MAX` elements. Physical storage is
//! governed by the immutable pointer width carried by the checking context. `length(b)` is an Int
//! in that range. A literal checks every entry against T. A read or update
//! requires kernel-checked evidence of `0 <= i && i < length(b)`. A push
//! requires evidence of `length(b) < usize::MAX`; ordinary executable push
//! supplies that fact only on normal return, after its capacity/allocator
//! failure paths have been accounted for by the exec checker. The buffer
//! language contains no unchecked read and no source annotation is allowed
//! to manufacture those facts.
//!
//! The reduction equations are structural: the length of a literal is its
//! entry count; updating keeps length; pushing adds one; reading a literal
//! selects its checked entry; reading an updated index yields the supplied
//! element; reading the pushed index yields the appended element. Rewrites
//! at different symbolic indices require the appropriate equality/ordering
//! evidence. Each update creates a new snapshot; old snapshots remain true
//! descriptions of old contents and cannot authorize access to new contents.
//!
//! Runtime indexing, update, allocation, push, and reference acquisition
//! have explicit exec IR operations. Their arguments run left to right.
//! Allocation/panic behavior remains runtime behavior; postconditions hold
//! only after normal return. Erasing a model observation never erases an
//! effectful argument. Rust foreign operations are admitted only by trusted
//! declarations recording an explicit reason, signature, promises, and
//! specification, all included in the audit.
//!
//! Runtime containers may carry erased logical payloads. Their length, tags,
//! storage operations and eager argument effects remain physical; Rust uses
//! `Vec<Erased>` or `[Erased; N]`, including Rust's zero-sized-element allocation
//! behavior. The exec operation carries `logical_payload`, derived from the
//! source erasure layout by the checked native factory. It may be true only
//! for an intrinsically logical type or the kernel Bool (which also models
//! source runtime bool). Element inputs then check in logical mode; storage
//! and indices still check in executable mode. This flag never grants a
//! logical value permission to control runtime computation: source layout
//! checking and the independently checked erased helper enforce that boundary.
//! The checked interpreter projects logical elements to the same marker as
//! the erased interpreter while retaining physical buffer structure.
//!
//! A library Seq model is related by element order and length. It is not
//! identified with Buffer by representation or assumed equal to an address.
//! Constructing/composing a model is logical computation, while observing
//! an existing physical value requires the short shared permission check
//! and records the current root version without retaining a Rust reference.

use super::check::{check_proof, expect_type, same, term_type, type_ok};
use super::{Context, KernelError, Mode, Term, Type};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferOp {
    Literal,
    Length,
    Get,
    Set,
    Push,
}

pub fn length(element: Type, value: Term) -> Term {
    Term::Buffer {
        op: BufferOp::Length,
        element,
        arguments: vec![value],
    }
}

pub fn bounds(element: &Type, value: &Term, index: &Term) -> [Term; 2] {
    [
        Term::int_le(Term::int(0), index.clone()),
        Term::int_lt(index.clone(), length(element.clone(), value.clone())),
    ]
}

pub(super) fn infer(
    ctx: &mut Context,
    op: BufferOp,
    element: &Type,
    args: &[Term],
    mode: Mode,
) -> Result<Type, KernelError> {
    // Collection snapshots are logical descriptions. Runtime operations
    // have explicit exec IR rules and cannot be smuggled through a term.
    if mode == Mode::Executable {
        return Err(KernelError::InvalidBuffer(
            "buffer observations require logical mode",
        ));
    }
    type_ok(ctx, element)?;
    let buffer = Type::Buffer(Box::new(element.clone()));
    if op == BufferOp::Literal {
        if super::Integer::from(args.len() as u128) > ctx.pointer_width().usize().max() {
            return Err(KernelError::InvalidBuffer(
                "buffer exceeds target usize::MAX",
            ));
        }
        for value in args {
            entry(ctx, value, element)?;
        }
        return Ok(buffer);
    }
    let arity = match op {
        BufferOp::Length => 1,
        BufferOp::Get => 4,
        BufferOp::Set => 5,
        BufferOp::Push => 3,
        BufferOp::Literal => unreachable!(),
    };
    if args.len() != arity {
        return Err(KernelError::WrongArity {
            expected: arity,
            found: args.len(),
        });
    }
    expect_type(ctx, &args[0], &buffer, Mode::Logical)?;
    match op {
        BufferOp::Length => Ok(Type::Int),
        BufferOp::Get | BufferOp::Set => {
            expect_type(ctx, &args[1], &Type::Int, Mode::Logical)?;
            let claims = bounds(element, &args[0], &args[1]);
            for (value, claim) in args[2..4].iter().zip(claims) {
                entry(ctx, value, &Type::proof(claim))?;
            }
            if op == BufferOp::Set {
                entry(ctx, &args[4], element)?;
                Ok(buffer)
            } else {
                Ok(element.clone())
            }
        }
        BufferOp::Push => {
            entry(ctx, &args[1], element)?;
            let room = Term::int_lt(
                length(element.clone(), args[0].clone()),
                Term::Int(ctx.pointer_width().usize().max()),
            );
            entry(ctx, &args[2], &Type::proof(room))?;
            Ok(buffer)
        }
        BufferOp::Literal => unreachable!(),
    }
}

/// One trusted structural computation step, after the complete input term
/// has been checked. No symbolic index is guessed or compared by a solver.
pub(super) fn step(term: &Term) -> Option<Term> {
    let Term::Buffer {
        op,
        element,
        arguments: args,
    } = term
    else {
        return None;
    };
    let first = args.first()?;
    match (op, first) {
        (
            BufferOp::Length,
            Term::Buffer {
                op: BufferOp::Literal,
                arguments,
                ..
            },
        ) => Some(Term::Int(super::Integer::from(arguments.len() as i128))),
        (
            BufferOp::Length,
            Term::Buffer {
                op: BufferOp::Set,
                arguments,
                ..
            },
        ) => Some(length(element.clone(), arguments.first()?.clone())),
        (
            BufferOp::Length,
            Term::Buffer {
                op: BufferOp::Push,
                arguments,
                ..
            },
        ) => Some(Term::int_add(
            length(element.clone(), arguments.first()?.clone()),
            Term::int(1),
        )),
        (
            BufferOp::Get,
            Term::Buffer {
                op: BufferOp::Literal,
                arguments,
                ..
            },
        ) => {
            let Term::Int(index) = args.get(1)? else {
                return None;
            };
            arguments
                .get(usize::try_from(index.to_i128()?).ok()?)
                .cloned()
        }
        (
            BufferOp::Get,
            Term::Buffer {
                op: BufferOp::Set,
                arguments,
                ..
            },
        ) if same(args.get(1)?, arguments.get(1)?) => arguments.get(4).cloned(),
        (
            BufferOp::Get,
            Term::Buffer {
                op: BufferOp::Push,
                arguments,
                ..
            },
        ) if same(
            args.get(1)?,
            &length(element.clone(), arguments.first()?.clone()),
        ) =>
        {
            arguments.get(1).cloned()
        }
        (
            BufferOp::Set,
            Term::Buffer {
                op: BufferOp::Literal,
                arguments,
                ..
            },
        ) => {
            let Term::Int(index) = args.get(1)? else {
                return None;
            };
            let mut values = arguments.clone();
            *values.get_mut(usize::try_from(index.to_i128()?).ok()?)? = args.get(4)?.clone();
            Some(Term::Buffer {
                op: BufferOp::Literal,
                element: element.clone(),
                arguments: values,
            })
        }
        (
            BufferOp::Push,
            Term::Buffer {
                op: BufferOp::Literal,
                arguments,
                ..
            },
        ) => {
            let mut values = arguments.clone();
            values.push(args.get(1)?.clone());
            Some(Term::Buffer {
                op: BufferOp::Literal,
                element: element.clone(),
                arguments: values,
            })
        }
        _ => None,
    }
}

pub(super) fn proof_step(ctx: &mut Context, term: &Term) -> Result<Term, KernelError> {
    let ty = term_type(ctx, term, Mode::Logical)?;
    if let Term::Eq(element, left, right) = term {
        if let (
            Term::Buffer {
                op: BufferOp::Get,
                element: le,
                arguments: la,
            },
            Term::Buffer {
                op: BufferOp::Get,
                element: re,
                arguments: ra,
            },
        ) = (&**left, &**right)
            && let Term::Buffer {
                op: BufferOp::Push,
                element: pe,
                arguments: pushed,
            } = &la[0]
            && super::check::same_type(element, le)
            && super::check::same_type(le, re)
            && super::check::same_type(le, pe)
            && same(&pushed[0], &ra[0])
            && same(&la[1], &ra[1])
        {
            return Ok(term.clone());
        }
        return Err(KernelError::InvalidBuffer(
            "read preservation needs matching checked reads before and after push",
        ));
    }
    if matches!(ty, Type::Proof(_)) {
        return Err(KernelError::EqualityAtProofType(ty));
    }
    let reduced = step(term).ok_or(KernelError::InvalidBuffer(
        "no buffer computation step applies",
    ))?;
    expect_type(ctx, &reduced, &ty, Mode::Logical)?;
    Ok(Term::eq(ty, term.clone(), reduced))
}

pub(super) fn proof_bound(
    ctx: &mut Context,
    value: &Term,
    upper: bool,
) -> Result<Term, KernelError> {
    let Type::Buffer(element) = term_type(ctx, value, Mode::Logical)? else {
        return Err(KernelError::InvalidBuffer(
            "length bounds need a buffer snapshot",
        ));
    };
    let len = length(*element, value.clone());
    Ok(if upper {
        Term::int_le(len, Term::Int(ctx.pointer_width().usize().max()))
    } else {
        Term::int_le(Term::int(0), len)
    })
}

// Proof-bearing positions stay canonical, as in tuple/struct constructors.
// This preserves the kernel's comparison-up-to-proof-irrelevance invariant.
fn entry(ctx: &mut Context, value: &Term, ty: &Type) -> Result<(), KernelError> {
    if let Type::Proof(claim) = ty {
        let Term::Proof(proof) = value else {
            return Err(KernelError::ProofExpected(value.clone()));
        };
        check_proof(ctx, proof, claim)
    } else {
        expect_type(ctx, value, ty, Mode::Logical)
    }
}
