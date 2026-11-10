// A permission is not `Copy`, so it cannot be used from two places.
use permissioned_pointers_spike::perm;

pub fn f() {
    let (p, perm) = perm::alloc::<u32>(1);
    let a = perm;
    let b = perm;
    perm::free(p, a, 1);
    perm::free(p, b, 1);
}
