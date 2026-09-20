//! Locus's syntax frontend, proof kernel, and check IR. The frontend is
//! independent of the other two:
//! parsing success is not type or proof checking, and the kernel checks
//! explicit terms that do not come from the parser yet.

pub mod ast;
pub mod diagnostic;
pub mod erased;
pub mod exec;
pub mod kernel;
pub mod lexer;
pub mod parser;
pub mod source;
pub mod typed;
