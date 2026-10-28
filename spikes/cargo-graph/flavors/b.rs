//! Lowered text of a Locus library `b` that depends on `a`. In the checking
//! program, each of lines 13 to 15 must be rejected (E0382): it observes `v`
//! after `v` has moved. In the runtime program they compile.
pub fn calls() -> u32 {
    let n = 6;
    let h = ::__locus::prove!(n);
    let _w: Option<::__locus::Erased> = a::witness();
    let p = a::Pos { v: 1, inv: ::__locus::ErasedData };
    a::div(n, 2, h) + p.v
}

// Probes: Locus's macro directly, through `$crate` of `a`, and through `::__locus` in `a`'s macro.
pub fn direct() { let v = vec![1]; drop(v); let _ = ::__locus::prove!(v); }
pub fn via_dollar() { let v = vec![1]; drop(v); let _ = a::assert_dollar!(v); }
pub fn via_abs() { let v = vec![1]; drop(v); let _ = a::assert_abs!(v); }
