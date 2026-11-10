//! Spike: can `Box` and `Vec` be written with no `unsafe` code, on top of a
//! small layer of permissioned pointers? See README.md.
//!
//! `perm` is the only module that may contain `unsafe`. `pbox` and `pvec`
//! forbid it.
#![deny(unsafe_code)]

pub mod pbox;
pub mod perm;
pub mod pvec;
