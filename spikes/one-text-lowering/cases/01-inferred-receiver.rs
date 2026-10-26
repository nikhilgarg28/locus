// expect: check=ok run=ok
struct Gate {
    min: u32,
}

impl Gate {
    fn admit(&self, n: u32, pos: ::__locus::Proof![n > 0]) -> ::__locus::Proof![n >= self.min] {
        if n < self.min {
            panic!("below the minimum");
        }
        ::__locus::proof!([] [])
    }
}

fn check(gates: &Vec<Gate>, n: u32, pos: ::__locus::Proof![n > 0]) -> u32 {
    let ok = gates.first().unwrap().admit(n, pos);
    let held = Some(ok);
    if held.is_some() { n } else { 0 }
}

fn main() {
    let gates = vec![Gate { min: 2 }];
    let r = check(&gates, 5, ::__locus::proof!([] []));
    println!("check(&gates, 5, _) = {r}");
}
#[allow(unused_imports)] #[macro_use] extern crate __locus; #[allow(unused_imports)] use __locus::prelude::*;
