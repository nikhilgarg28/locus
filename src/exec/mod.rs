//! The check IR and the exec checker (Architecture in atlas.html).
//!
//! The check IR is executable code in let-normal form over kernel types, terms, and
//! proofs. It exists to be checked: `lower` will produce it from the typed
//! tree, and nothing prints or runs it in production. The exec checker walks
//! it while maintaining a kernel context, which is how a program path becomes
//! the kernel statements of specification section 6.3. It is trusted.

mod buffer;
mod dynamic;
pub use dynamic::{DynInterface, DynMethod, DynTable, DynTableId};
mod check;
mod interp;
mod ir;
pub(crate) mod projection;

pub use crate::erased::Overflow;
pub use check::{ExecError, Program, TrustedContract};
pub use interp::{CheckInterpreter, Lending};
pub use ir::{
    Arm, Block, BufferStmt, BufferStorage, ExecFn, ExecFnId, ForStmt, OperateStmt, Promise,
    Promises, Stmt, Tail,
};
