// `free` consumes the permission, so a later use is a use of a moved value.
use permissioned_pointers_spike::perm;

pub fn f() -> u32 {
    let (p, mut perm) = perm::alloc::<u32>(1);
    perm::write(p, &mut perm, 0, 5);
    let _ = perm::take(p, &mut perm, 0);
    perm::free(p, perm, 1);
    *perm::get(p, &perm, 0)
}
