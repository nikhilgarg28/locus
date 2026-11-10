// Two live mutable borrows of one slot need two mutable borrows of its
// permission.
use permissioned_pointers_spike::perm;

pub fn f() {
    let (p, mut perm) = perm::alloc::<u32>(1);
    perm::write(p, &mut perm, 0, 5);
    let a = perm::get_mut(p, &mut perm, 0);
    let b = perm::get_mut(p, &mut perm, 0);
    *a += *b;
}
