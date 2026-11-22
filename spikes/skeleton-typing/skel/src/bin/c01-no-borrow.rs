//! expect: runtime=ok check=ok
//! prints: run() => ([1, 2, 3], "locus::Seq<locus::BoundedU32>", "c01_no_borrow::__locus_snap_Link")
//! A ghost snapshot keeps no borrow. One of a `&mut`, held across a new
//! mutable use of the place. One of a struct with a lifetime parameter,
//! held after the struct and what it borrowed are gone: the generated
//! snapshot type has no lifetime.
use ::locus::locus;

locus! {
    struct Link<'a> { payload: &'a u32, parent: &'a u32 }
    fn keep<G: Logical>(g: G) -> G { g }
    fn type_of<T>(_: &T) -> &'static str { std::any::type_name::<T>() }

    fn run() -> (Vec<u32>, &'static str, &'static str) {
        let mut v = vec![1u32];
        let r = &mut v;
        let before = logic! { snap(r) };
        r.push(2);
        v.push(3);
        let ghost = {
            let x = 7u32;
            let link = Link { payload: &x, parent: &x };
            logic! { snap(link) }
        };
        let unchanged = prove!(before.len() + 2 == v.len());
        (v, type_of(&keep(before)), type_of(&keep(ghost)))
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
