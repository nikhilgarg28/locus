//! expect: runtime=E0599 check=E0599
//! A logic function over a type's snapshot, declared in `impl Stack` with a
//! `Snap<Stack>` parameter, is called by its path, `Stack::room(snap(s))`
//! (a02). As a method on the snapshot, `snap(s).room()`, it is not found:
//! the toy writes the stub on `Stack`, and the snapshot is another type.
use ::locus::locus;
locus! {
    struct Stack { items: Vec<u32>, cap: usize }
    impl Stack {
        logic fn room(s: Snap<Stack>) -> Int { s.cap - s.items.len() }
    }
    fn run() -> u32 {
        let s = Stack { items: vec![], cap: 2 };
        let by_path = logic! { Stack::room(snap(s)) };
        let as_method = logic! { snap(s).room() };
        1
    }
}
fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
