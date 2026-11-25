//! expect: runtime=ok check=ok
//! prints: same() => true
//! A logic function of a trait with the name of a method that runs. The
//! stub is `__logic_eq`, so it is no candidate when rustc resolves the `eq`
//! that Rust code wrote.
use ::locus::locus;

locus! {
    trait EqSpec {
        logic fn eq(a: Self, b: Self) -> Bool;
    }

    #[derive(PartialEq, Clone, Copy)]
    struct Version { major: u32 }

    impl EqSpec for Version {
        logic fn eq(a: Version, b: Version) -> Bool { a.major == b.major }
    }

    fn same() -> bool {
        let (a, b) = (Version { major: 3 }, Version { major: 3 });
        let spec = prop!(Version::eq(a, b));
        Version::eq(&a, &b) && a.eq(&b)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("same() => {:?}", same());
}
