//! expect: runtime=reject check=reject
//! "A value whose type has no model cannot be used [as a value]; `snap(x)`
//! gives its snapshot instead" (RFC 0002). rustc says so, as an unmet
//! bound; its fields are still read through their own models.
use ::locus::locus;

locus! {
    struct Token { id: u32 }

    fn run(t: &Token, u: &Token) -> u32 {
        let fields = prove!(t.id == u.id);
        let snapshots = prove!(snap(t) === snap(u));
        let values = prove!(t === u);
        t.id
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run(&Token { id: 1 }, &Token { id: 1 }));
}
