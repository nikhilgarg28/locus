//! Runs the two collections through their paths. Under `--cfg checked` every
//! primitive asserts its `requires`, so a pass means the collections met
//! every contract on these paths, and the counts show nothing leaked.

use permissioned_pointers_spike::pbox::PBox;
use permissioned_pointers_spike::pvec::PVec;
use std::cell::Cell;
use std::rc::Rc;

/// Adds one to a shared counter when dropped.
struct Counted(Rc<Cell<usize>>, u32);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn boxes() {
    let mut b = PBox::new(String::from("locus"));
    b.push_str(" 1");
    assert_eq!(&*b, "locus 1");
    assert_eq!(b.into_inner(), "locus 1");

    let drops = Rc::new(Cell::new(0));
    drop(PBox::new(Counted(drops.clone(), 0)));
    assert_eq!(drops.get(), 1);
    let kept = PBox::new(Counted(drops.clone(), 7)).into_inner();
    assert_eq!((drops.get(), kept.1), (1, 7));
    drop(kept);
    assert_eq!(drops.get(), 2);
}

fn vectors() {
    let mut v = PVec::new();
    for i in 0..1000u32 {
        v.push(i.to_string());
    }
    assert_eq!(v.len(), 1000);
    assert_eq!(v[999], "999");
    v[3].push('!');
    assert_eq!(v[3], "3!");
    assert_eq!(v.pop().as_deref(), Some("999"));
    assert_eq!(v.swap_remove(0), "0");
    assert_eq!(v[0], "998");
    assert_eq!(v.iter().filter(|s| s.len() == 3).count(), 899);

    let drops = Rc::new(Cell::new(0));
    let mut c = PVec::new();
    for i in 0..10 {
        c.push(Counted(drops.clone(), i));
    }
    drop(c.pop());
    assert_eq!(drops.get(), 1);
    let mut it = c.into_iter();
    assert_eq!(it.next().map(|x| x.1), Some(0));
    assert_eq!(drops.get(), 2);
    drop(it);
    assert_eq!(drops.get(), 10);

    // Zero-sized elements and an empty vector never reach the allocator.
    let mut z = PVec::new();
    for _ in 0..10 {
        z.push(());
    }
    assert_eq!((z.len(), z.pop()), (10, Some(())));
    let e: PVec<u64> = PVec::new();
    assert!(e.is_empty());
}

/// Compiles only if the collections are covariant in their element type.
#[allow(dead_code)]
fn covariant<'a>(b: PBox<&'static str>, v: PVec<&'static str>) -> (PBox<&'a str>, PVec<&'a str>) {
    (b, v)
}

fn auto_traits() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PBox<u32>>();
    send_sync::<PVec<String>>();
}

#[cfg(not(checked))]
fn sizes() {
    use permissioned_pointers_spike::perm::Perm;
    use std::mem::size_of;
    let word = size_of::<usize>();
    assert_eq!(size_of::<Perm<u64>>(), 0);
    assert_eq!(size_of::<PBox<u64>>(), word);
    assert_eq!(size_of::<Option<PBox<u64>>>(), word);
    assert_eq!(size_of::<PVec<u64>>(), 3 * word);
    assert_eq!(size_of::<Option<PVec<u64>>>(), 3 * word);
    println!(
        "sizes: Perm 0, PBox {word}, Option<PBox> {word}, PVec {}",
        3 * word
    );
}

#[test]
fn everything() {
    boxes();
    vectors();
    auto_traits();
    #[cfg(not(checked))]
    sizes();
    #[cfg(checked)]
    {
        let (allocated, freed, leaked) = permissioned_pointers_spike::perm::counts();
        println!("checked: {allocated} blocks allocated, {freed} freed, {leaked} leaked");
        assert!(allocated > 0);
        assert_eq!((allocated - freed, leaked), (0, 0));
    }
}
