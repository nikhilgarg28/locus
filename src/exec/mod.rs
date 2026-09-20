//! The check IR and the exec checker (docs/ir-architecture.md).
//!
//! The check IR is executable code in let-normal form over kernel types, terms, and
//! proofs. It exists to be checked: `lower` will produce it from the typed
//! tree, and nothing prints or runs it in production. The exec checker walks
//! it while maintaining a kernel context, which is how a program path becomes
//! the kernel statements of specification section 6.3. It is trusted.

mod check;
mod ir;

pub use check::{ExecError, Program};
pub use ir::{Arm, Block, ExecFn, ExecFnId, ForStmt, Stmt, Tail};
