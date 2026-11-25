//! A stand-in for Locus's library as RFC 0001 has it today:
//! one marker, `LogicalData`, for every logical value that is not a proof.
#![allow(dead_code)]
pub trait Logical: Copy {}
#[derive(Clone, Copy, Debug)]
pub struct LogicalData;
impl Logical for LogicalData {}
#[derive(Clone, Copy, Debug)]
pub struct Erased;
impl Logical for Erased {}
pub type Int = LogicalData;
pub type Nat = LogicalData;
