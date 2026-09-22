//! Locus's syntax frontend, proof kernel, and check IR. The frontend is
//! independent of the other two:
//! parsing success is not type or proof checking, and the kernel checks
//! explicit terms that do not come from the parser yet. The arithmetic
//! procedure in `arith` searches for certificates the kernel checks; it is
//! outside the kernel and untrusted.

pub mod arith;
pub mod ast;
pub mod build;
pub mod diagnostic;
pub mod elab;
pub mod erased;
pub mod exec;
pub mod kernel;
pub mod lexer;
pub mod parser;
pub mod source;
pub mod store;
pub mod typed;
