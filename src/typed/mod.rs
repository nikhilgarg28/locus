//! The typed tree and its lowering (Architecture in atlas.html).
//!
//! The typed tree is the source program made fully explicit, in the shape of
//! the source. `lower` turns it into what is checked: kernel declarations for
//! structs, enums, and math functions, and check IR for ordinary functions.
//! `lower` is trusted. The typed tree itself is only a claim until its
//! lowering is accepted.

mod lower;
mod tree;

pub use lower::{FnRef, LowerError, Session, is_pure, value_term};
pub use tree::{
    Binder, Block, CompareOp, EnumItem, Expr, FnItem, MatchArm, Pattern, Stmt, StructItem,
    VariantItem,
};
