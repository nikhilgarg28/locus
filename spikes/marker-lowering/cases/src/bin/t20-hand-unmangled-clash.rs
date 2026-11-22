//! expect: runtime=E0034 check=E0034
//! Plain Rust, written by hand: t19 as it would be lowered if the stub kept
//! the logic function's name. rustc then has two candidates for the `eq`
//! that Rust code wrote.
use ::locus::{Bool, stub};

trait EqSpecLogic {
    fn eq(a: Self, b: Self) -> Bool
    where
        Self: Sized;
}
trait EqSpec: EqSpecLogic {}

#[derive(PartialEq, Clone, Copy)]
struct Version {
    major: u32,
}

impl EqSpecLogic for Version {
    fn eq(_a: Version, _b: Version) -> Bool {
        stub()
    }
}
impl EqSpec for Version {}

fn same() -> bool {
    let (a, b) = (Version { major: 3 }, Version { major: 3 });
    Version::eq(&a, &b)
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("same() => {:?}", same());
}
