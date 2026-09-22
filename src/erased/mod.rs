//! The erased tree (Architecture in atlas.html): the typed tree with the logic
//! taken out. It has the same shape, and contains only what exists at
//! runtime. A ghost position is filled by a zero-sized marker, `Proved` or
//! `Ghost`, so nothing is renumbered or moved.
//!
//! `erase` produces it and is trusted. The type checker here is a cheap
//! guard on `erase`: everything it emits must be well typed in a simple type
//! system with no propositions. The interpreter is the reference semantics
//! and a test oracle; it is not trusted. The Rust printer turns the erased
//! tree into source for rustc, and is trusted.

mod check;
mod erase;
mod interp;
mod rust;
mod tree;

pub use check::{TypeError, check_module};
pub use erase::{erase_enum, erase_fn, erase_struct, erase_type};
pub use interp::{Interpreter, Outcome, Overflow, RunError, Value};
pub(crate) use interp::{Stop, operate, outcome, term_value, value_term};
pub use rust::{
    MARKERS, Markers, Visibilities, print_items, print_module, print_module_with, print_root,
};
pub use tree::{
    EArm, EBlock, EEnum, EExpr, EFn, EPattern, EPlace, EStmt, EStruct, EType, EVariant, Module,
};
