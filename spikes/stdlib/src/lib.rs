//! Executable library-design experiment, not a Locus compiler or proof checker.
//! `audit` stores executable shadow models. Disable it to exercise the same
//! runtime algorithms without those shadow allocations. Neither mode proves laws.
pub mod btree;
pub mod checked;
pub mod hash;
pub mod interior;
pub mod logic;
pub mod permission;
pub mod sharing;
pub mod vector;
pub mod wrappers;
