//! Locus's library for the spike's third variant, "split", crate `locus`.
//!
//! The two expansions differ. Under `--cfg __locus_check` the library is the
//! first mode's: a zero-sized type for each logical type (`checking.rs`).
//! Without it, the library is the one-marker library, and every type has
//! every trait whose items are all logical (`running.rs`). `locus!` lowers
//! each item both ways, and `__when_checking!` and `__when_running!` keep
//! one of the two.

// `locus!` writes `::locus::..` paths, and the library's own logical traits
// are written in Locus, through `locus!`.
extern crate self as locus;

#[cfg(__locus_check)]
#[path = "checking.rs"]
mod half;
#[cfg(not(__locus_check))]
#[path = "running.rs"]
mod half;

pub use half::*;

/// The items of the checking expansion.
#[macro_export]
macro_rules! __when_checking {
    ($($i:item)*) => {
        $(
            #[cfg(__locus_check)]
            $i
        )*
    };
}
/// The items of the runtime expansion.
#[macro_export]
macro_rules! __when_running {
    ($($i:item)*) => {
        $(
            #[cfg(not(__locus_check))]
            $i
        )*
    };
}
