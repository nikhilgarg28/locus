//! The typed tree and its lowering (Architecture in atlas.html).
//!
//! The typed tree is the source program made fully explicit, in the shape of
//! the source. `lower` turns it into what is checked: kernel declarations for
//! structs, enums, and math functions, and check IR for ordinary functions.
//! `lower` is trusted. The typed tree itself is only a claim until its
//! lowering is accepted.

mod lower;
mod tree;

pub(crate) use lower::visit_block as each_stmt;
pub use lower::{
    FnRef, LowerError, Named, Session, block_leaves, is_pure, join_type, opened_part, opened_type,
    rebuilt, value_term,
};
pub(crate) use lower::{each_expr, visit_expr as each_stmt_under};
pub use tree::{
    Binder, Block, Carried, CompareOp, Derive, EnumItem, Expr, FnItem, Join, Joined, Lend,
    MatchArm, PanicForm, Passing, Pattern, Place, Step, Stmt, StructItem, VariantItem,
};
