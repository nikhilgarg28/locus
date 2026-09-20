//! Locus's syntax frontend and proof kernel. The two are independent:
//! parsing success is not type or proof checking, and the kernel checks
//! explicit terms that do not come from the parser yet.

pub mod ast;
pub mod diagnostic;
pub mod kernel;
pub mod lexer;
pub mod parser;
pub mod source;
