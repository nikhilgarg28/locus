// A slice borrowed from the block keeps its permission borrowed.
use permissioned_pointers_spike::perm;

pub fn f() -> u32 {
    let (p, mut perm) = perm::alloc::<u32>(2);
    perm::write(p, &mut perm, 0, 5);
    let s = perm::slice(p, &perm, 1);
    perm::write(p, &mut perm, 1, 6);
    s[0]
}
